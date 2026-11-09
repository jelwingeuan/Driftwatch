use driftwatch::collect::tcc::read_db;
use rusqlite::Connection;

#[test]
fn reads_synthetic_tcc_db() {
    let dir = std::env::temp_dir().join(format!("driftwatch tcc {}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("TCC.db");
    let conn = Connection::open(&db).unwrap();
    conn.execute_batch(
        "CREATE TABLE access (service TEXT, client TEXT, client_type INTEGER, auth_value INTEGER,
                              indirect_object_identifier TEXT DEFAULT 'UNUSED');
         INSERT INTO access (service, client, client_type, auth_value)
             VALUES ('kTCCServiceCamera', 'com.example.cam', 0, 2);
         INSERT INTO access (service, client, client_type, auth_value)
             VALUES ('kTCCServiceMicrophone', '/usr/local/bin/rec', 1, 0);
         INSERT INTO access VALUES ('kTCCServiceAppleEvents', 'com.example.cam', 0, 2, 'com.apple.finder');",
    )
    .unwrap();
    drop(conn);
    let garbage = dir.join("broken.db");
    std::fs::write(&garbage, b"not a database").unwrap();

    let items = read_db(&db, "user");
    let broken = read_db(&garbage, "system");
    let missing = read_db(&dir.join("missing.db"), "system");
    std::fs::remove_dir_all(&dir).unwrap();

    assert_eq!(items.len(), 3);
    assert_eq!(
        items[0].key,
        "user: kTCCServiceCamera com.example.cam (type 0)"
    );
    assert!(!items[0].fields.contains_key("indirect_object"));
    assert_eq!(
        items[2].key,
        "user: kTCCServiceAppleEvents com.example.cam (type 0) -> com.apple.finder"
    );
    assert_eq!(items[0].fields["auth_value"], "2");
    assert_eq!(items[1].fields["client"], "/usr/local/bin/rec");
    assert_eq!(broken.len(), 1);
    assert!(broken[0].fields["error"].starts_with("needs Full Disk Access"));
    assert!(missing.is_empty());
}
