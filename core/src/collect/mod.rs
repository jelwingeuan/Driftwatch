use crate::Item;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

pub mod apps;
pub mod btm;
pub mod certs;
pub mod cron;
pub mod launchd;
pub mod network;
pub mod shell;
pub mod ssh;
pub mod sysext;
pub mod tcc;

/// How long a tool may run before it is stopped: long enough to type the admin
/// password `sfltool dumpbtm` asks for.
pub(crate) const TOOL_LIMIT: Duration = Duration::from_secs(120);

/// The most a collector reads from one file.
const FILE_LIMIT: u64 = 16 << 20;

/// Run a tool (stopped after `TOOL_LIMIT`) and return its stdout, or a one-line
/// error naming what failed. `cmd` is an absolute path, so PATH never picks the tool.
pub(crate) fn run(cmd: &str, args: &[&str]) -> Result<String, String> {
    let out = output(Command::new(cmd).args(args), TOOL_LIMIT)?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let last = err.lines().last().unwrap_or("").trim();
        let name = tool_name(cmd);
        return Err(format!("{name} failed ({}): {last}", out.status));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// A tool's file name: errors say "security", not "/usr/bin/security", as before
/// tools were run by absolute path, so stored error items don't show as changed.
fn tool_name(program: &str) -> &str {
    program.rsplit('/').next().unwrap_or(program)
}

/// Run `cmd` with no input, waiting at most `limit`. Err (naming the tool) when it
/// can't start or runs too long, in which case it is killed.
pub(crate) fn output(cmd: &mut Command, limit: Duration) -> Result<Output, String> {
    let program = cmd.get_program().to_string_lossy();
    let name = tool_name(&program).to_string();
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{name}: {e}"))?;
    // Drain both pipes while waiting, so a chatty tool can't fill one and stall.
    fn drain(mut pipe: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = pipe.read_to_end(&mut buf);
            buf
        })
    }
    let stdout = drain(child.stdout.take().expect("stdout is piped"));
    let stderr = drain(child.stderr.take().expect("stderr is piped"));
    let deadline = Instant::now() + limit;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "{name} took longer than {}s and was stopped",
                    limit.as_secs()
                ));
            }
            Err(e) => return Err(format!("{name}: {e}")),
        }
    };
    Ok(Output {
        status,
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
}

/// A file's bytes, read so nothing on disk can stall or flood the scan: opened with
/// O_NOFOLLOW | O_NONBLOCK, so a pipe or device can't block, and only a regular
/// file of at most 16 MiB is read. A symlink (dotfiles often are) is resolved
/// first and its target gets the same checks.
pub(crate) fn read_file(path: &Path) -> io::Result<Vec<u8>> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(std::fs::canonicalize(path)?)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("not a regular file"));
    }
    let mut bytes = Vec::new();
    file.take(FILE_LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > FILE_LIMIT {
        return Err(io::Error::other("larger than 16 MiB"));
    }
    Ok(bytes)
}

/// `read_file` as UTF-8 text.
pub(crate) fn read_text(path: &Path) -> io::Result<String> {
    String::from_utf8(read_file(path)?).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "stream did not contain valid UTF-8",
        )
    })
}

/// An Item that records why something could not be read.
pub(crate) fn error_item(category: &str, key: &str, error: &str) -> Item {
    let fields = BTreeMap::from([("error".to_string(), error.to_string())]);
    Item::new(category, key, fields)
}

/// Hash a file's bytes so any edit shows up. `None` when the file does not exist;
/// an error Item when it exists but cannot be read.
pub(crate) fn file_item(category: &str, path: &Path) -> Option<Item> {
    let key = path.to_string_lossy();
    match read_file(path) {
        Ok(bytes) => Some(Item::new(
            category,
            &key,
            BTreeMap::from([
                ("file_sha256".into(), crate::hex(&Sha256::digest(&bytes))),
                ("size".into(), bytes.len().to_string()),
            ]),
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => Some(error_item(category, &key, &e.to_string())),
    }
}

/// Sorted paths in `dir` (hidden files included). Missing dir → empty. An
/// unreadable dir or entry is pushed to `errors` as an error Item, never dropped.
pub(crate) fn list_dir(category: &str, dir: &Path, errors: &mut Vec<Item>) -> Vec<PathBuf> {
    let key = dir.to_string_lossy();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            errors.push(error_item(category, &key, &e.to_string()));
            return Vec::new();
        }
    };
    let mut paths = Vec::new();
    for (i, entry) in entries.enumerate() {
        match entry {
            Ok(e) => paths.push(e.path()),
            Err(e) => errors.push(error_item(
                category,
                &format!("{key}#entry{i}"),
                &e.to_string(),
            )),
        }
    }
    paths.sort();
    paths
}

/// Like `list_dir`, minus subdirectories (symlinks are followed, so links to files stay).
pub(crate) fn list_files(category: &str, dir: &Path, errors: &mut Vec<Item>) -> Vec<PathBuf> {
    let mut paths = list_dir(category, dir, errors);
    paths.retain(|p| !p.is_dir());
    paths
}

#[cfg(test)]
mod tests {
    use super::output;
    use std::process::Command;
    use std::time::{Duration, Instant};

    #[test]
    fn a_tool_that_runs_too_long_is_stopped_with_an_error() {
        let start = Instant::now();
        let err = output(Command::new("sleep").arg("5"), Duration::from_secs(1)).unwrap_err();
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "{:?}",
            start.elapsed()
        );
        assert_eq!(err, "sleep took longer than 1s and was stopped");

        let ok = output(Command::new("echo").arg("hi"), Duration::from_secs(5)).unwrap();
        assert!(ok.status.success());
        assert_eq!(ok.stdout, b"hi\n");
    }

    #[test]
    fn tools_run_by_absolute_path_are_named_by_file_name() {
        let missing = output(
            &mut Command::new("/nonexistent/tool"),
            Duration::from_secs(5),
        );
        assert!(missing.unwrap_err().starts_with("tool: "));
        let failed = super::run("/usr/bin/false", &[]).unwrap_err();
        assert!(failed.starts_with("false failed ("), "{failed}");
    }
}
