use driftwatch::collect::shell::collect_files;
use std::path::PathBuf;

#[test]
fn hashes_present_files_and_skips_missing() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shell");
    let items = collect_files(
        &[root.join("zshrc"), root.join("missing")],
        &[root.join("paths.d"), root.join("no-such-dir")],
    );
    assert_eq!(items.len(), 2);
    assert!(items[0].key.ends_with("shell/zshrc"));
    assert_eq!(items[0].fields["size"], "30");
    assert!(items[1].key.ends_with("paths.d/10-example"));
    assert_eq!(items[1].fields["size"], "17");
    assert_ne!(items[0].hash, items[1].hash);
}
