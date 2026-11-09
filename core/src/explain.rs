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

/// Programs that run code they are handed.
const SHELLS: [&str; 9] = [
    "sh",
    "bash",
    "zsh",
    "osascript",
    "python",
    "python3",
    "perl",
    "ruby",
    "node",
];

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
                Some(n) if !SHELLS.contains(&n) => n.to_string(),
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

/// Why a command line looks like malware, if it does.
fn suspicious(cmd: &str) -> Option<&'static str> {
    let c = cmd.to_lowercase();
    let piped_to_shell = c.split('|').skip(1).any(|seg| {
        seg.split_whitespace()
            .next()
            .is_some_and(|t| SHELLS.contains(&file_name(t)))
    });
    if (c.contains("curl") || c.contains("wget")) && piped_to_shell {
        Some("Downloads code from the internet and runs it straight away, a common malware trick.")
    } else if c.contains("dig ") && c.contains("txt") {
        Some("Reads hidden instructions from a DNS record, a trick used by malware.")
    } else if c.contains("base64") && (c.contains(" -d") || c.contains("--decode")) {
        Some("Runs hidden (base64-encoded) code.")
    } else {
        None
    }
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
    match item.category.as_str() {
        "launchd" => {
            let runner = field(item, "Program")
                .or(command(item))?
                .split_whitespace()
                .next()?;
            let runner = file_name(runner);
            if SHELLS.contains(&runner) || runner == "curl" || runner == "wget" {
                command(item).and_then(suspicious).map(str::to_string)
            } else {
                None
            }
        }
        "cron" if field(item, "source").is_some() => suspicious(&item.key).map(str::to_string),
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
