use driftwatch::collect::network::{parse_dns, parse_hosts};

#[test]
fn parses_hosts_lines() {
    let keys: Vec<String> = parse_hosts(include_str!("fixtures/hosts"))
        .into_iter()
        .map(|i| i.key)
        .collect();
    assert_eq!(
        keys,
        [
            "hosts: 127.0.0.1 localhost",
            "hosts: 255.255.255.255 broadcasthost",
            "hosts: ::1 localhost",
            "hosts: 203.0.113.7 update.example.com",
        ]
    );
}

#[test]
fn parses_scutil_dns() {
    let items = parse_dns(include_str!("fixtures/scutil-dns.txt"));
    let keys: Vec<&str> = items.iter().map(|i| i.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "dns resolver #1: 192.168.0.1",
            "dns resolver #1: 192.168.68.1",
            "dns scoped resolver #1: 192.168.0.1",
            "dns scoped resolver #1: 192.168.68.1",
        ]
    );
    assert_eq!(items[0].fields["nameserver"], "192.168.0.1");
}
