use driftwatch::collect::shell::collect_files;
use std::path::PathBuf;

#[test]
fn hashes_present_files_and_skips_missing() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shell");
    let items = collect_files(
        &[root.join("zshrc"), root.join("missing")],
        &[root.join("paths.d"), root.join("no-such-dir")],
    );
    // paths.d/subdir is skipped; the 20-link symlink to a file is followed.
    assert_eq!(items.len(), 3);
    assert!(items[0].key.ends_with("shell/zshrc"));
    assert_eq!(items[0].fields["size"], "30");
    assert!(items[1].key.ends_with("paths.d/10-example"));
    assert_eq!(items[1].fields["size"], "17");
    assert_ne!(items[0].hash, items[1].hash);
    assert!(items[2].key.ends_with("paths.d/20-link"));
    assert_eq!(
        items[2].fields["file_sha256"],
        items[1].fields["file_sha256"]
    );
    assert!(items.iter().all(|i| !i.fields.contains_key("error")));
}

#[test]
fn unreadable_dir_becomes_error_item() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("driftwatch-locked-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o000)).unwrap();
    let items = collect_files(&[], std::slice::from_ref(&dir));
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::remove_dir(&dir).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].key, dir.to_string_lossy());
    assert!(items[0].fields.contains_key("error"));
}
