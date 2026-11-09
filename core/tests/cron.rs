use driftwatch::collect::cron::{collect_periodic, parse_crontab};
use std::path::PathBuf;

#[test]
fn parses_crontab_lines() {
    let items = parse_crontab(include_str!("fixtures/crontab.txt"));
    let keys: Vec<&str> = items.iter().map(|i| i.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "SHELL=/bin/sh",
            "*/5 * * * * /usr/local/bin/backup.sh --quiet",
            "@reboot /Users/example/bin/start.sh",
        ]
    );
    assert!(parse_crontab("").is_empty());
}

#[test]
fn hashes_periodic_scripts() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/periodic");
    let items = collect_periodic(&[root.join("daily"), root.join("weekly")]);
    assert_eq!(items.len(), 1);
    assert!(items[0].key.ends_with("daily/100.example"));
    assert_eq!(items[0].fields["size"], "21");
    assert_eq!(items[0].fields["file_sha256"].len(), 64);
}
