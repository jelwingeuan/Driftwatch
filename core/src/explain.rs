//! Plain words for an Item: a title, one sentence, a risk hint and who probably
//! added it. Offline: per-category templates plus the bundled catalog.json.
use crate::Item;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Explanation {
    pub title: String,
    pub summary: String,
    /// "low", "medium" or "high": a hint, never a verdict.
    pub risk: &'static str,
    pub risk_reason: Option<String>,
    pub added_by: Option<String>,
    /// "known" (from the code signature) or "likely" (from a name or path).
    pub confidence: Option<&'static str>,
}

#[derive(Deserialize)]
struct Catalog {
    teams: BTreeMap<String, String>,
    prefixes: BTreeMap<String, String>,
    domains: Vec<String>,
}

fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../catalog.json")).expect("catalog.json is valid")
    })
}

/// Programs that run code they are handed (versioned names like python3.12 count too).
const INTERPRETERS: [&str; 14] = [
    "sh",
    "bash",
    "zsh",
    "dash",
    "ksh",
    "tcsh",
    "csh",
    "fish",
    "pwsh",
    "osascript",
    "python",
    "perl",
    "ruby",
    "node",
];

/// Commands that fetch from the network, matched as whole words.
const FETCH_WORDS: [&str; 6] = ["curl", "wget", "nc", "ncat", "netcat", "requests"];

/// Text that means a network fetch wherever it appears.
const FETCH_TEXT: [&str; 10] = [
    "urllib",
    "http.client",
    "socket.socket",
    "/dev/tcp/",
    "/dev/udp/",
    "lwp::",
    "io::socket",
    "open-uri",
    "net::http",
    "net/http",
];

/// Quoting and grouping around a command word.
const QUOTES: [char; 5] = ['"', '\'', '`', '(', ')'];

const DNS_CODE: &str =
    "Reads hidden instructions from a DNS record and runs them, a trick used by malware.";
const DOWNLOADED_CODE: &str =
    "Downloads code from the internet and runs it straight away, a common malware trick.";
const HIDDEN_CODE: &str = "Runs hidden (base64-encoded) code.";
const REMOTE_SHELL: &str = "Lets someone on the internet run commands on your Mac.";

/// (TCC service, what it lets an app do), worded to follow "<app> can".
const TCC_SERVICES: [(&str, &str); 20] = [
    ("kTCCServiceCamera", "use your camera"),
    ("kTCCServiceMicrophone", "use your microphone"),
    ("kTCCServiceScreenCapture", "record your screen"),
    ("kTCCServiceSystemPolicyAllFiles", "read all your files"),
    ("kTCCServiceAccessibility", "control your Mac"),
    ("kTCCServiceListenEvent", "see what you type"),
    ("kTCCServicePostEvent", "type and click for you"),
    ("kTCCServiceAppleEvents", "control other apps"),
    ("kTCCServiceAddressBook", "see your contacts"),
    ("kTCCServiceCalendar", "see your calendar"),
    ("kTCCServiceReminders", "see your reminders"),
    ("kTCCServicePhotos", "see your photos"),
    (
        "kTCCServiceSystemPolicyDesktopFolder",
        "open your Desktop folder",
    ),
    (
        "kTCCServiceSystemPolicyDocumentsFolder",
        "open your Documents folder",
    ),
    (
        "kTCCServiceSystemPolicyDownloadsFolder",
        "open your Downloads folder",
    ),
    ("kTCCServiceSystemPolicyRemovableVolumes", "open USB drives"),
    (
        "kTCCServiceSystemPolicyNetworkVolumes",
        "open network drives",
    ),
    ("kTCCServiceMediaLibrary", "see your music and media"),
    ("kTCCServiceSpeechRecognition", "use speech recognition"),
    (
        "kTCCServiceDeveloperTool",
        "run software without the usual checks",
    ),
];

/// TCC grants that give an app wide reach.
const POWERFUL_TCC: [&str; 4] = [
    "kTCCServiceSystemPolicyAllFiles",
    "kTCCServiceAccessibility",
    "kTCCServiceScreenCapture",
    "kTCCServiceListenEvent",
];

pub fn explain(item: &Item) -> Explanation {
    let (added_by, confidence) = match attribute(item) {
        Some((who, how)) => (Some(who), Some(how)),
        None => (None, None),
    };
    let (risk, risk_reason) = risk(item, added_by.as_deref());
    Explanation {
        title: title(item),
        summary: summary(item),
        risk,
        risk_reason,
        added_by,
        confidence,
    }
}

/// A plain label for a raw field name; unknown names come back unchanged.
pub fn field_label(name: &str) -> &str {
    match name {
        "ProgramArguments" => "What it runs",
        "Program" | "executable" => "Program",
        "RunAtLoad" => "Starts at login",
        "KeepAlive" => "Restarts if stopped",
        "StartInterval" => "Runs every (seconds)",
        "UserName" => "Runs as user",
        "Label" | "name" | "label" => "Name",
        "scope" => "Where",
        "file_sha256" => "File contents",
        "size" => "Size (bytes)",
        "team_id" => "Developer ID",
        "sha1" => "Fingerprint",
        "developer" => "Developer",
        "type" => "Kind",
        "disposition" => "Status",
        "identifier" => "Identifier",
        "url" => "Location",
        "bundle_id" => "App ID",
        "parent" => "Part of",
        "uid" => "User ID",
        "keychain" => "Keychain",
        "trust" => "Trust setting",
        "service" => "Permission",
        "client" => "App",
        "client_type" => "App kind",
        "auth_value" => "Allowed",
        "indirect_object" => "Target",
        "source" => "Source",
        "nameserver" => "DNS server",
        "domain" => "Domain",
        "options" => "Key options",
        "comment" => "Comment",
        "mtime" => "Last changed",
        "authority" => "Signed by",
        "cdhash" => "Code fingerprint",
        "signature" => "Signature",
        "signed" => "Signed",
        "version" => "Version",
        "enabled" => "Turned on",
        "active" => "Running",
        "state" => "State",
        "error" => "Problem",
        other => other,
    }
}

fn field<'a>(item: &'a Item, name: &str) -> Option<&'a str> {
    item.fields
        .get(name)
        .map(String::as_str)
        .filter(|v| !v.is_empty())
}

fn file_name(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}

fn shorten(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max - 1).collect();
    format!("{cut}…")
}

/// A BTM item's identifier (kept in its key) without the numeric type prefix ("16.").
fn btm_id(item: &Item) -> &str {
    let id = item
        .key
        .split_once(": ")
        .map_or(item.key.as_str(), |(_, id)| id);
    match id.split_once('.') {
        Some((n, rest)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => rest,
        _ => id,
    }
}

/// What a launch item runs: ProgramArguments, else Program.
fn command(item: &Item) -> Option<&str> {
    field(item, "ProgramArguments").or(field(item, "Program"))
}

/// "hosts: <ip> <names…>" → (ip, names).
fn hosts_line(item: &Item) -> Option<(&str, Vec<&str>)> {
    let mut parts = item.key.strip_prefix("hosts: ")?.split_whitespace();
    let ip = parts.next()?;
    Some((ip, parts.take_while(|t| !t.starts_with('#')).collect()))
}

fn is_authorized_key(item: &Item) -> bool {
    item.category == "ssh" && item.key.contains("authorized_keys") && field(item, "type").is_some()
}

fn title(item: &Item) -> String {
    let get = |name| field(item, name);
    let best = if get("error").is_some() {
        None
    } else {
        match item.category.as_str() {
            "launchd" => get("Label").map(str::to_string),
            "btm" => Some(match get("name") {
                // A bare interpreter name ("bash") says nothing; the label does.
                Some(n) if !is_interpreter(n) => n.to_string(),
                _ => file_name(btm_id(item)).to_string(),
            }),
            "certs" => get("label").map(str::to_string),
            "tcc" => tcc_title(item),
            "network" => hosts_line(item)
                .map(|(ip, names)| format!("{} → {ip}", names.join(", ")))
                .or_else(|| get("nameserver").map(|ns| format!("DNS server {ns}"))),
            "ssh" if is_authorized_key(item) => Some(match get("comment") {
                Some(c) => format!("Remote login key ({c})"),
                None => "Remote login key".to_string(),
            }),
            "apps" => Some(file_name(&item.key).trim_end_matches(".app").to_string()),
            "sysext" => get("name").map(str::to_string),
            _ => None,
        }
    };
    best.unwrap_or_else(|| {
        if item.key.starts_with('/') {
            file_name(&item.key).to_string()
        } else {
            item.key.clone()
        }
    })
}

fn tcc_title(item: &Item) -> Option<String> {
    let client = field(item, "client")?;
    let service = field(item, "service")?;
    let app = if client.starts_with('/') {
        file_name(client)
    } else {
        client.rsplit('.').next().unwrap_or(client)
    };
    let what = TCC_SERVICES
        .iter()
        .find(|(s, _)| *s == service)
        .map_or_else(
            || format!("use {}", service.trim_start_matches("kTCCService")),
            |(_, w)| w.to_string(),
        );
    let can = if field(item, "auth_value") == Some("0") {
        "can’t"
    } else {
        "can"
    };
    Some(format!("{app} {can} {what}"))
}

fn with_command(sentence: &str, cmd: Option<&str>) -> String {
    match cmd {
        Some(cmd) => format!("{sentence} It runs: {}", shorten(cmd, 80)),
        None => sentence.to_string(),
    }
}

fn summary(item: &Item) -> String {
    let get = |name| field(item, name);
    if get("error").is_some() {
        return "Driftwatch couldn’t read this item.".to_string();
    }
    match item.category.as_str() {
        "launchd" => {
            let when = match get("scope") {
                Some("global daemon") => {
                    "Runs a program in the background as the system (root) when your Mac starts."
                }
                Some("global agent") => "Starts a program for every user who logs in.",
                _ => "Starts a program every time you log in.",
            };
            with_command(when, command(item))
        }
        "btm" => {
            let kind = get("type").unwrap_or("");
            if kind.starts_with("developer") {
                return "Groups the login and background items from one developer.".to_string();
            }
            let what = if kind.starts_with("app") {
                "An app that may open at login or run in the background."
            } else if kind.contains("daemon") {
                "Runs a program in the background as the system."
            } else if kind.contains("agent") || kind.starts_with("login item") {
                "Starts a helper program when you log in."
            } else {
                "A background part of an app."
            };
            let mut s = with_command(what, get("executable"));
            if get("disposition").is_some_and(|d| d.contains("disabled")) {
                s.push_str(" It’s turned off.");
            }
            s
        }
        "certs" if is_root_trusted(item) => {
            "A certificate your Mac was told to always trust for secure connections.".to_string()
        }
        "certs" => "A security certificate stored in your keychain.".to_string(),
        "tcc" => "A privacy permission from System Settings › Privacy & Security.".to_string(),
        "network" => match hosts_line(item) {
            Some((ip, names)) => format!(
                "Sends your Mac to {ip} whenever it looks up {}.",
                names.join(", ")
            ),
            None if get("nameserver").is_some() => {
                "A server your Mac asks to look up web addresses.".to_string()
            }
            None => format!(
                "Sends lookups for {} addresses to a custom DNS server.",
                file_name(&item.key)
            ),
        },
        "ssh" if is_authorized_key(item) => {
            "Lets whoever holds the matching private key log in to your Mac remotely.".to_string()
        }
        "ssh" if item.key.contains("sshd_config") => {
            "Settings for remote login to this Mac.".to_string()
        }
        "ssh" if item.key.ends_with("/config") => {
            "Settings for SSH connections you make to other computers.".to_string()
        }
        "ssh" if item.key.ends_with(".pub") => "One of your SSH public keys.".to_string(),
        "ssh" => "A file in your SSH folder (Driftwatch never reads its contents).".to_string(),
        "apps" if get("signed") == Some("no") => {
            "An app in your Applications folder. It isn’t signed by a developer.".to_string()
        }
        "apps" => "An app in your Applications folder.".to_string(),
        "shell" if item.key.starts_with("/etc/paths") => {
            "Lists the folders your Mac searches for commands.".to_string()
        }
        "shell" => "A file your Terminal runs each time it starts a shell.".to_string(),
        "cron" if get("source").is_some() => {
            with_command("Runs a command on a schedule.", Some(&item.key))
        }
        "cron" => "A maintenance script macOS runs on a schedule.".to_string(),
        "sysext" => {
            "An add-on that works deep inside macOS, like a network filter or security tool."
                .to_string()
        }
        _ => "Something Driftwatch watches on your Mac.".to_string(),
    }
}

fn is_root_trusted(item: &Item) -> bool {
    field(item, "trust").is_some_and(|t| {
        t.contains("TrustRoot")
            || t.contains("TrustAsRoot")
            || t.split([';', '|'])
                .any(|s| s.trim().ends_with("Number of trust settings : 0"))
    })
}

/// A command word's program without folder or version: "/usr/bin/python3.12" → "python".
fn program_name(word: &str) -> &str {
    file_name(word.trim_matches(QUOTES)).trim_end_matches(|c: char| c.is_ascii_digit() || c == '.')
}

fn is_interpreter(word: &str) -> bool {
    INTERPRETERS.contains(&program_name(word))
}

/// Where `word` first appears in `text` with no letter, digit, `_` or `-` either side.
fn word_at(text: &str, word: &str) -> Option<usize> {
    let part = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    text.match_indices(word)
        .map(|(i, _)| i)
        .find(|&i| !text[..i].ends_with(part) && !text[i + word.len()..].starts_with(part))
}

fn fetches(text: &str) -> bool {
    FETCH_WORDS.iter().any(|w| word_at(text, w).is_some())
        || FETCH_TEXT.iter().any(|t| text.contains(t))
}

/// Why running what `text` produces is risky: it comes from a DNS record, the
/// network, or base64. Expects lowercase text.
fn code_source(text: &str) -> Option<&'static str> {
    let dns = word_at(text, "dig").is_some_and(|i| word_at(&text[i..], "txt").is_some());
    let base64 = text.contains("b64decode")
        || word_at(text, "base64").is_some()
            && text
                .split_whitespace()
                .any(|w| matches!(w.trim_matches(QUOTES), "-d" | "--decode"));
    if dns {
        Some(DNS_CODE)
    } else if fetches(text) {
        Some(DOWNLOADED_CODE)
    } else if base64 {
        Some(HIDDEN_CODE)
    } else {
        None
    }
}

/// The text inside each `$(…)`, `<(…)` and pair of backticks.
fn substitutions(cmd: &str) -> impl Iterator<Item = &str> + '_ {
    let parens = cmd
        .match_indices("$(")
        .chain(cmd.match_indices("<("))
        .map(|(i, _)| {
            let inner = &cmd[i + 2..];
            let mut depth = 0;
            for (j, c) in inner.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' if depth == 0 => return &inner[..j],
                    ')' => depth -= 1,
                    _ => {}
                }
            }
            inner
        });
    cmd.split('`').skip(1).step_by(2).chain(parens)
}

/// Whether a command runs code: an interpreter (also behind sudo, env or xargs),
/// or chmod making a file runnable.
fn runs_code(command: &str) -> bool {
    let mut words = command.split_whitespace().map(|w| w.trim_matches(QUOTES));
    match words.next().map(file_name) {
        Some("sudo" | "env" | "xargs") => words.any(is_interpreter),
        Some("chmod") => words.any(|w| {
            w.contains("+x")
                || w.len() >= 3
                    && w.bytes().all(|b| b.is_ascii_digit() && b < b'8')
                    && w.bytes().any(|b| b % 2 == 1)
        }),
        Some(w) => is_interpreter(w),
        None => false,
    }
}

/// Why inline code (`python3 -c`, `osascript -e`, `perl -e`, `ruby -e`, `node -e`)
/// is risky, if it fetches or decodes code.
fn inline_code(words: &[&str]) -> Option<&'static str> {
    words.iter().enumerate().find_map(|(i, w)| {
        let flag = match program_name(w) {
            "python" => "-c",
            "osascript" | "perl" | "ruby" | "node" => "-e",
            _ => return None,
        };
        // Options come first; osascript's `-l JavaScript` takes a value.
        let mut j = i + 1;
        while let Some(o) = words.get(j).filter(|o| o.starts_with('-') && **o != flag) {
            j += if *o == "-l" { 2 } else { 1 };
        }
        (words.get(j) == Some(&flag))
            .then(|| code_source(&words[i + 1..].join(" ")))
            .flatten()
    })
}

/// Why a command line looks like malware, if it does: it runs code fetched from
/// the network, a DNS record or base64, or wires a shell to the network. Looks
/// at the whole text, whatever program comes first.
fn suspicious(cmd: &str) -> Option<&'static str> {
    let c = cmd.to_lowercase();
    let words: Vec<&str> = c
        .split_whitespace()
        .map(|w| w.trim_matches(QUOTES))
        .collect();
    // `bash -i >& /dev/tcp/…`, `nc … -e /bin/sh`.
    let remote_shell = (c.contains("/dev/tcp/") || c.contains("/dev/udp/"))
        && words.iter().any(|w| is_interpreter(w))
        || fetches(&c)
            && words.windows(2).any(|w| {
                is_interpreter(w[0]) && w[1] == "-i" || w[0] == "-e" && is_interpreter(w[1])
            });
    if remote_shell {
        return Some(REMOTE_SHELL);
    }
    // `sh -c "$(curl …)"`, `bash <(curl …)`, `` eval `wget …` ``.
    if let Some(reason) = substitutions(&c).find_map(code_source) {
        return Some(reason);
    }
    if let Some(reason) = inline_code(&words) {
        return Some(reason);
    }
    // A fetch or decode, then a later command that runs code: `| sudo bash`,
    // `&& sh /tmp/x`, `; chmod +x /tmp/x`.
    let commands: Vec<&str> = c.split([';', '|', '&', '\n']).collect();
    commands.iter().enumerate().find_map(|(i, part)| {
        let reason = code_source(part)?;
        commands[i + 1..]
            .iter()
            .any(|later| runs_code(later))
            .then_some(reason)
    })
}

/// The command text an item runs, for `suspicious`: a launch item's Program and
/// ProgramArguments, a BTM item's executable and url, a crontab line. Shell
/// startup files and periodic scripts are only hashed, so there is no text.
fn command_text(item: &Item) -> Option<String> {
    let names = match item.category.as_str() {
        "launchd" => ["Program", "ProgramArguments"],
        "btm" => ["executable", "url"],
        "cron" if field(item, "source").is_some() => return Some(item.key.clone()),
        _ => return None,
    };
    let parts: Vec<&str> = names.iter().filter_map(|f| field(item, f)).collect();
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// Why the folder a launch or BTM item's program sits in is odd, if it is: the
/// program, a script handed to an interpreter, or a BTM item's path.
fn odd_folder(item: &Item) -> Option<&'static str> {
    let mut paths: Vec<&str> = ["Program", "executable", "url"]
        .iter()
        .filter_map(|f| field(item, f))
        .collect();
    if let Some(args) = field(item, "ProgramArguments") {
        let mut words = args.split_whitespace();
        let program = words.next();
        paths.extend(program);
        if program.is_some_and(is_interpreter) {
            paths.extend(words.find(|w| !w.starts_with('-')));
        }
    }
    paths.into_iter().find_map(|path| {
        let path = path.trim_matches(QUOTES).to_lowercase();
        let path = path.strip_prefix("file://").unwrap_or(&path);
        if path.starts_with("/tmp/") || path.starts_with("/private/tmp/") {
            Some("Runs a program from a temporary folder, where installed software doesn’t live.")
        } else if path.starts_with("/users/shared/") {
            Some("Runs a program from the Shared folder, which anyone on this Mac can change.")
        } else {
            let (_, in_home) = path.strip_prefix("/users/")?.split_once('/')?;
            let (folders, _) = in_home.rsplit_once('/')?;
            folders
                .split('/')
                .any(|f| f.starts_with('.'))
                .then_some("Runs a program from a hidden folder, a place malware likes to use.")
        }
    })
}

fn risk(item: &Item, vendor: Option<&str>) -> (&'static str, Option<String>) {
    if field(item, "error").is_some() {
        return ("low", None);
    }
    if let Some(reason) = high_risk(item) {
        return ("high", Some(reason));
    }
    match medium_risk(item, vendor) {
        Some(reason) => ("medium", Some(reason.to_string())),
        None => ("low", None),
    }
}

fn high_risk(item: &Item) -> Option<String> {
    if let Some(reason) = command_text(item).as_deref().and_then(suspicious) {
        return Some(reason.to_string());
    }
    match item.category.as_str() {
        "certs" if is_root_trusted(item) => Some(
            "Your Mac fully trusts this added certificate, so whoever owns it could read or fake secure websites."
                .to_string(),
        ),
        "network" => {
            let (ip, names) = hosts_line(item)?;
            let blocks = ["0.0.0.0", "::", "::1", "255.255.255.255"].contains(&ip)
                || ip.starts_with("127.")
                || ip.starts_with("fe80::1");
            let name = names.iter().find(|n| {
                let n = n.to_lowercase();
                catalog()
                    .domains
                    .iter()
                    .any(|d| n == *d || n.ends_with(&format!(".{d}")))
            })?;
            (!blocks).then(|| {
                format!("Sends {name} to a different address than the real one, which could fake that website.")
            })
        }
        "ssh" if is_authorized_key(item) => {
            Some("Anyone with the matching key can log in to your Mac.".to_string())
        }
        _ => None,
    }
}

fn medium_risk(item: &Item, vendor: Option<&str>) -> Option<&'static str> {
    match item.category.as_str() {
        "launchd" | "btm" => {
            if let Some(reason) = odd_folder(item) {
                return Some(reason);
            }
            let daemon = field(item, "scope") == Some("global daemon")
                || field(item, "type").is_some_and(|t| t.contains("daemon"));
            if daemon && vendor != Some("Apple") {
                Some("Runs with full control of your Mac (as root).")
            } else if vendor.is_none() && item.team_id.is_none() {
                Some("Driftwatch can’t tell who made this.")
            } else {
                None
            }
        }
        "tcc" => {
            let service = field(item, "service").unwrap_or("");
            (POWERFUL_TCC.contains(&service) && field(item, "auth_value") != Some("0"))
                .then_some("This permission lets an app see or control a lot of your Mac.")
        }
        "apps" if field(item, "signed") == Some("no") => {
            Some("This app isn’t signed, so macOS can’t check who made it.")
        }
        _ => None,
    }
}

/// Who probably added the item: from the signer ("known"), else from its
/// identifier or an /Applications path ("likely").
fn attribute(item: &Item) -> Option<(String, &'static str)> {
    let cat = catalog();
    let team = item.team_id.as_deref().or(field(item, "team_id"));
    if let Some(vendor) = team.and_then(|t| cat.teams.get(t)) {
        return Some((vendor.clone(), "known"));
    }
    if field(item, "authority") == Some("Software Signing") {
        return Some(("Apple".to_string(), "known"));
    }
    // BTM reads the developer's name from the code signature.
    if let Some(dev) = field(item, "developer") {
        return Some((dev.to_string(), "known"));
    }
    let key_id = match item.category.as_str() {
        "btm" => Some(btm_id(item)),
        "sysext" => item.key.split_whitespace().next(),
        _ => None,
    };
    let ids = ["Label", "bundle_id", "identifier", "client"]
        .iter()
        .filter_map(|f| field(item, f))
        .chain(key_id);
    for id in ids {
        let id = id.to_lowercase();
        if let Some((_, vendor)) = cat.prefixes.iter().find(|(p, _)| id.starts_with(*p)) {
            return Some((vendor.clone(), "likely"));
        }
    }
    for name in ["Program", "ProgramArguments", "executable", "url"] {
        let Some(v) = field(item, name) else { continue };
        if let Some(i) = v.find("Applications/") {
            if let Some(end) = v[i..].find(".app") {
                return Some((file_name(&v[i..i + end]).to_string(), "likely"));
            }
        }
    }
    None
}
