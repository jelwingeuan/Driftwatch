use driftwatch::collect::launchd::collect_dirs;
use std::path::PathBuf;

fn fixtures(sub: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(sub)
}

#[test]
fn reads_xml_binary_and_broken_plists() {
    let items = collect_dirs(&[
        (fixtures("agents"), "user agent"),
        (fixtures("daemons"), "global daemon"),
        (fixtures("missing"), "global agent"),
    ]);
    // notes.txt is skipped, the missing dir is skipped, broken.plist is kept.
    assert_eq!(items.len(), 4);
    assert!(items.iter().all(|i| i.fields["file_sha256"].len() == 64));

    let both = &items[0];
    assert!(both.key.ends_with("agents/com.example.both.plist"));
    assert_eq!(both.fields["Program"], "/usr/bin/env");
    assert_eq!(both.fields["ProgramArguments"], "env -i");

    let xml = &items[1];
    assert!(xml.key.ends_with("agents/com.example.xml.plist"));
    assert_eq!(xml.fields["Label"], "com.example.xml");
    assert_eq!(xml.fields["ProgramArguments"], "/usr/bin/true --flag");
    assert!(!xml.fields.contains_key("Program"));
    assert_eq!(xml.fields["RunAtLoad"], "true");
    assert_eq!(xml.fields["StartInterval"], "300");
    assert_eq!(xml.fields["scope"], "user agent");

    let broken = &items[2];
    assert!(broken.key.ends_with("daemons/broken.plist"));
    assert!(broken.fields.contains_key("error"));
    assert_eq!(broken.fields["scope"], "global daemon");

    let bin = &items[3];
    assert_eq!(bin.fields["Label"], "com.example.binary");
    assert_eq!(bin.fields["Program"], "/usr/local/bin/helper");
    assert_eq!(bin.fields["KeepAlive"], "SuccessfulExit=false");
    assert_eq!(bin.fields["UserName"], "root");
}

#[test]
fn untracked_key_change_changes_hash() {
    let dir = std::env::temp_dir().join(format!("driftwatch-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("com.example.watch.plist");
    let write = |path: &str| {
        let xml = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\"><dict>\
             <key>Label</key><string>com.example.watch</string>\
             <key>WatchPaths</key><array><string>{path}</string></array>\
             </dict></plist>"
        );
        std::fs::write(&file, xml).unwrap();
    };
    let scan = || collect_dirs(&[(dir.clone(), "user agent")]).remove(0);

    write("/tmp/a");
    let before = scan();
    write("/tmp/b");
    let after = scan();
    std::fs::remove_dir_all(&dir).unwrap();

    assert_eq!(before.fields["Label"], after.fields["Label"]);
    assert_ne!(before.hash, after.hash);
}
