use driftwatch::collect::{btm, certs, cron, launchd, shell, sysext, tcc};
use driftwatch::diff::{diff, ChangeKind};
use driftwatch::store::{self, Store};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("scan") {
        eprintln!("usage: driftwatch scan");
        std::process::exit(2);
    }
    if let Err(e) = scan() {
        eprintln!("driftwatch: {e}");
        std::process::exit(1);
    }
}

fn scan() -> Result<(), Box<dyn std::error::Error>> {
    let mut items = launchd::collect();
    items.extend(btm::collect());
    items.extend(cron::collect());
    items.extend(shell::collect());
    items.extend(sysext::collect());
    items.extend(tcc::collect());
    items.extend(certs::collect());
    let mut store = Store::open(&store::default_path().map_err(|_| "HOME is not set")?)?;
    let previous = store.latest_items()?;
    store.save_snapshot(&items)?;
    let Some(previous) = previous else {
        println!("Baseline saved: {} items", items.len());
        return Ok(());
    };
    let changes = diff(&previous, &items);
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
