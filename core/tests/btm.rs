use driftwatch::collect::btm::parse;

#[test]
fn parses_sfltool_dumpbtm() {
    let text = include_str!("fixtures/sfltool-dumpbtm.txt");
    let items = parse(text);
    assert_eq!(items.len(), 3);

    let dev = &items[0];
    assert_eq!(dev.key, "uid -2: Adobe Creative Cloud");
    assert_eq!(dev.fields["type"], "developer (0x20)");
    assert!(!dev.fields.contains_key("url"));
    assert_eq!(dev.team_id, None);

    let daemon = &items[1];
    assert_eq!(daemon.key, "uid -2: 16.com.adobe.acc.installer.v2");
    assert_eq!(daemon.team_id.as_deref(), Some("JQ525L2MZD"));
    assert_eq!(
        daemon.fields["disposition"],
        "[enabled, disallowed, notified] (0x9)"
    );
    assert_eq!(daemon.fields["parent"], "Adobe Creative Cloud");
    assert_eq!(
        daemon.fields["executable"],
        "/Library/PrivilegedHelperTools/com.adobe.acc.installer.v2"
    );
    assert_eq!(daemon.fields["uid"], "-2");

    // Same item in another UID: own key, and Generation differences do not change the hash.
    assert_eq!(items[2].key, "uid 0: Adobe Creative Cloud");
    assert_eq!(items[2].fields["uid"], "0");
    let mut a = dev.fields.clone();
    a.remove("uid");
    let mut b = items[2].fields.clone();
    b.remove("uid");
    assert_eq!(a, b);
}
