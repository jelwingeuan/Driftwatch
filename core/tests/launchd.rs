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
    assert_eq!(items.len(), 3);

    let xml = &items[0];
    assert!(xml.key.ends_with("agents/com.example.xml.plist"));
    assert_eq!(xml.fields["Label"], "com.example.xml");
    assert_eq!(xml.fields["Program"], "/usr/bin/true --flag");
    assert_eq!(xml.fields["RunAtLoad"], "true");
    assert_eq!(xml.fields["StartInterval"], "300");
    assert_eq!(xml.fields["scope"], "user agent");

    let broken = &items[1];
    assert!(broken.key.ends_with("daemons/broken.plist"));
    assert!(broken.fields.contains_key("error"));
    assert_eq!(broken.fields["scope"], "global daemon");

    let bin = &items[2];
    assert_eq!(bin.fields["Label"], "com.example.binary");
    assert_eq!(bin.fields["Program"], "/usr/local/bin/helper");
    assert_eq!(bin.fields["KeepAlive"], "SuccessfulExit=false");
    assert_eq!(bin.fields["UserName"], "root");
}
