use driftwatch::collect::certs::{parse_certs, parse_trust};

#[test]
fn parses_certs_with_trust() {
    let trust = parse_trust(
        include_str!("fixtures/security-dump-trust-settings.txt"),
        "user",
    );
    assert_eq!(trust.len(), 2);
    assert_eq!(
        trust[0].1,
        "user: Number of trust settings : 1; Policy OID : Apple X509 Basic; \
         Allowed Error : CSSMERR_TP_CERT_EXPIRED; \
         Result Type : kSecTrustSettingsResultTrustAsRoot"
    );

    let items = parse_certs(
        include_str!("fixtures/security-find-certificate.txt"),
        &trust,
    );
    assert_eq!(items.len(), 3);
    let proxy = &items[0];
    assert_eq!(
        proxy.key,
        "/Users/example/Library/Keychains/login.keychain-db AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
    );
    assert_eq!(proxy.fields["label"], "Example Corp Proxy CA");
    assert!(proxy.fields["keychain"].ends_with("login.keychain-db"));
    assert!(proxy.fields["trust"].contains("TrustAsRoot"));
    assert_eq!(
        items[1].fields["keychain"],
        "/Library/Keychains/System.keychain"
    );
    assert!(!items[1].fields.contains_key("trust"));
    // Trust for a cert not in the scanned keychains is still reported.
    assert_eq!(items[2].key, "trust: Orphan Root");
    assert_eq!(
        items[2].fields["trust"],
        "user: Number of trust settings : 0"
    );
}
