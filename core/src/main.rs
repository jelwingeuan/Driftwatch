use driftwatch::collect::{apps, btm, certs, cron, launchd, network, shell, ssh, sysext, tcc};
use driftwatch::diff::{diff, ChangeKind};
use driftwatch::report;
use driftwatch::store::{self, Store};
use driftwatch::Collector;
use std::error::Error;

const USAGE: &str = "usage: driftwatch scan [--json] [--skip CATEGORY[,CATEGORY...]]\n       driftwatch history --json [--limit N]";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json = args.iter().any(|a| a == "--json");
    let result = match (args.first().map(String::as_str), json) {
        (Some("scan"), _) => match skips(&args) {
            Some(skip) => scan(json, &skip),
            None => usage(),
        },
        (Some("history"), true) => match limit(&args) {
            Some(n) => history(n),
            None => usage(),
        },
        _ => usage(),
    };
    if let Err(e) = result {
        eprintln!("driftwatch: {e}");
        std::process::exit(1);
    }
}

fn usage() -> ! {
    eprintln!("{USAGE}");
    std::process::exit(2);
}

/// `--limit N` (default 100); None when N is missing or not a number.
fn limit(args: &[String]) -> Option<usize> {
    match args.iter().position(|a| a == "--limit") {
        None => Some(100),
        Some(i) => args.get(i + 1)?.parse().ok(),
    }
}

/// The categories after each `--skip` (repeatable, or comma-separated); None
/// when one is missing or not a category.
fn skips(args: &[String]) -> Option<Vec<&'static str>> {
    let mut skip = Vec::new();
    for (i, _) in args.iter().enumerate().filter(|(_, a)| *a == "--skip") {
        for name in args.get(i + 1)?.split(',') {
            skip.push(COLLECTORS.iter().find(|(c, _)| *c == name)?.0);
        }
    }
    Some(skip)
}

fn open_store() -> Result<Store, Box<dyn Error>> {
    Ok(Store::open(
        &store::default_path().map_err(|_| "HOME is not set")?,
    )?)
}

fn history(limit: usize) -> Result<(), Box<dyn Error>> {
    println!("{}", report::history(&open_store()?, limit)?);
    Ok(())
}

const COLLECTORS: [Collector; 10] = [
    (launchd::CATEGORY, |_| launchd::collect()),
    (btm::CATEGORY, |_| btm::collect()),
    (cron::CATEGORY, |_| cron::collect()),
    (shell::CATEGORY, |_| shell::collect()),
    (sysext::CATEGORY, |_| sysext::collect()),
    (tcc::CATEGORY, |_| tcc::collect()),
    (certs::CATEGORY, |_| certs::collect()),
    (network::CATEGORY, |_| network::collect()),
    (ssh::CATEGORY, |_| ssh::collect()),
    (apps::CATEGORY, apps::collect),
];

fn scan(json: bool, skip: &[&str]) -> Result<(), Box<dyn Error>> {
    let mut store = open_store()?;
    let previous = store.latest_items()?;
    let items = driftwatch::collect_all(&COLLECTORS, previous.as_deref().unwrap_or_default(), skip);
    let snapshot = store.save_snapshot(&items)?;
    let changes = previous
        .as_deref()
        .map_or_else(Vec::new, |p| diff(p, &items));
    if json {
        let baseline = previous.is_none();
        println!("{}", report::scan_json(&snapshot, baseline, &changes));
        return Ok(());
    }
    if previous.is_none() {
        println!("Baseline saved: {} items", items.len());
        return Ok(());
    }
    if changes.is_empty() {
        println!("No changes");
    }
    for c in changes {
        let (sign, word) = match c.kind {
            ChangeKind::Added => ('+', "added"),
            ChangeKind::Removed => ('-', "removed"),
            ChangeKind::Modified => ('~', "modified"),
        };
        let item = c
            .new
            .as_ref()
            .or(c.old.as_ref())
            .expect("change has an item");
        let detail = match (item.fields.get("Label"), item.fields.get("error")) {
            (Some(label), _) => format!("  (Label: {label})"),
            (None, Some(err)) => format!("  (error: {err})"),
            _ => String::new(),
        };
        println!("{sign} {word}  {}  {}{detail}", item.category, item.key);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::skips;

    fn args(s: &str) -> Vec<String> {
        s.split(' ').map(String::from).collect()
    }

    #[test]
    fn skip_takes_repeats_and_comma_lists_of_known_categories() {
        assert_eq!(skips(&args("scan --json")), Some(vec![]));
        assert_eq!(
            skips(&args("scan --skip btm,tcc --json --skip apps")),
            Some(vec!["btm", "tcc", "apps"])
        );
        assert_eq!(skips(&args("scan --skip")), None);
        assert_eq!(skips(&args("scan --skip --json")), None);
        assert_eq!(skips(&args("scan --skip login")), None);
    }
}
