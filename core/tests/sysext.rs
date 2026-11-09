use driftwatch::collect::sysext::parse;

#[test]
fn parses_systemextensionsctl_list() {
    let items = parse(include_str!("fixtures/systemextensionsctl-list.txt"));
    assert_eq!(items.len(), 2);
    let vpn = &items[0];
    assert_eq!(vpn.key, "com.example.vpn.extension ABCDE12345");
    assert_eq!(vpn.team_id.as_deref(), Some("ABCDE12345"));
    assert_eq!(vpn.fields["version"], "1.2.3/45");
    assert_eq!(vpn.fields["state"], "activated enabled");
    assert_eq!(vpn.fields["enabled"], "true");
    assert_eq!(
        vpn.fields["type"],
        "com.apple.system_extension.network_extension"
    );
    assert_eq!(items[1].fields["active"], "false");
    assert_eq!(
        items[1].fields["state"],
        "terminated waiting to uninstall on reboot"
    );
    assert!(parse("0 extension(s)\n").is_empty());
}
