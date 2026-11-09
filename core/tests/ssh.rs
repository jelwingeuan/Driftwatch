use driftwatch::collect::ssh::collect_ssh_dir;
use std::path::PathBuf;

#[test]
fn reads_ssh_dir_without_private_key_contents() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ssh");
    let items = collect_ssh_dir(&dir);
    // 3 authorized keys + config + private key presence + .pub; known_hosts skipped.
    assert_eq!(items.len(), 6);

    let ed = &items[0];
    assert!(ed.key.contains("authorized_keys: ssh-ed25519 "));
    assert_eq!(ed.fields["comment"], "alice@laptop");
    let rsa = &items[1];
    assert_eq!(rsa.fields["type"], "ssh-rsa");
    assert_eq!(rsa.fields["options"], "from=\"10.0.0.1\",no-pty");
    assert_eq!(rsa.fields["comment"], "backup key");
    assert!(!items[2].fields.contains_key("comment"));

    assert!(items[3].key.ends_with("ssh/config"));
    assert!(items[3].fields.contains_key("file_sha256"));
    let private = &items[4];
    assert!(private.key.ends_with("ssh/id_example"));
    assert_eq!(private.fields.keys().collect::<Vec<_>>(), ["mtime"]);
    assert!(items[5].key.ends_with("id_example.pub"));
    assert!(items[5].fields.contains_key("file_sha256"));
}
