use driftwatch::collect::{apps, btm, certs, cron, launchd, network, shell, ssh, sysext, tcc};
use driftwatch::diff::{diff, ChangeKind};
use driftwatch::report;
use driftwatch::store::{self, Store};
use std::error::Error;

const USAGE: &str = "usage: driftwatch scan [--json]\n       driftwatch history --json [--limit N]";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json = args.iter().any(|a| a == "--json");
    let result = match (args.first().map(String::as_str), json) {
        (Some("scan"), _) => scan(json),
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

fn open_store() -> Result<Store, Box<dyn Error>> {
    Ok(Store::open(
        &store::default_path().map_err(|_| "HOME is not set")?,
    )?)
}

fn history(limit: usize) -> Result<(), Box<dyn Error>> {
    println!("{}", report::history(&open_store()?, limit)?);
    Ok(())
}

fn scan(json: bool) -> Result<(), Box<dyn Error>> {
    let mut items = launchd::collect();
    items.extend(btm::collect());
    items.extend(cron::collect());
    items.extend(shell::collect());
    items.extend(sysext::collect());
    items.extend(tcc::collect());
    items.extend(certs::collect());
    items.extend(network::collect());
    items.extend(ssh::collect());
    items.extend(apps::collect());
    driftwatch::uniquify_keys(&mut items);
    let mut store = open_store()?;
    let previous = store.latest_items()?;
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
