use driftwatch::collect::apps::parse_codesign;

#[test]
fn parses_codesign_output() {
    let f = parse_codesign(include_str!("fixtures/codesign-stats.txt"));
    assert_eq!(f["identifier"], "eu.exelban.Stats");
    assert_eq!(f["team_id"], "RP2S87B72W");
    assert_eq!(
        f["authority"],
        "Developer ID Application: Serhiy Mytrovtsiy (RP2S87B72W)"
    );
    assert_eq!(f["cdhash"], "2e9a74bbcd706345aeccaf0b6e80e561d4cf0b82");

    let adhoc = parse_codesign("Identifier=app.example\nSignature=adhoc\nTeamIdentifier=not set\n");
    assert_eq!(adhoc["signature"], "adhoc");
    assert!(!adhoc.contains_key("team_id"));
}
