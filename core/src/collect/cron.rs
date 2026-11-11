use super::{error_item, file_item, list_files, output, TOOL_LIMIT};
use crate::Item;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

pub const CATEGORY: &str = "cron";

/// The current user's crontab lines plus the scripts in /etc/periodic.
pub fn collect() -> Vec<Item> {
    let mut items = match output(Command::new("/usr/bin/crontab").arg("-l"), TOOL_LIMIT) {
        Ok(out) if out.status.success() => parse_crontab(&String::from_utf8_lossy(&out.stdout)),
        // "no crontab for <user>" is the normal empty case.
        Ok(out) if String::from_utf8_lossy(&out.stderr).contains("no crontab") => Vec::new(),
        Ok(out) => vec![error_item(
            CATEGORY,
            "crontab -l",
            String::from_utf8_lossy(&out.stderr).trim(),
        )],
        Err(e) => vec![error_item(CATEGORY, "crontab -l", &e)],
    };
    let dirs = ["daily", "weekly", "monthly"].map(|d| PathBuf::from("/etc/periodic").join(d));
    items.extend(collect_periodic(&dirs));
    items
}

/// One Item per active crontab line (comments and blanks skipped). Key = the line.
pub fn parse_crontab(text: &str) -> Vec<Item> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| {
            let fields = BTreeMap::from([("source".to_string(), "user crontab".to_string())]);
            Item::new(CATEGORY, l, fields)
        })
        .collect()
}

/// Hash every file in the given periodic dirs. Key = file path.
pub fn collect_periodic(dirs: &[PathBuf]) -> Vec<Item> {
    let mut items = Vec::new();
    for dir in dirs {
        let paths = list_files(CATEGORY, dir, &mut items);
        items.extend(paths.iter().filter_map(|p| file_item(CATEGORY, p)));
    }
    items
}
