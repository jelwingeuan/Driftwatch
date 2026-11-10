use driftwatch::collect::apps::{app_item, parse_codesign};
use driftwatch::explain::explain;
use std::process::Command;

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

#[test]
fn app_whose_signature_breaks_after_signing_fails_the_check() {
    let dir = std::env::temp_dir().join(format!("driftwatch-apps-{}", std::process::id()));
    let app = dir.join("X.app");
    std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
    std::fs::copy("/usr/bin/true", app.join("Contents/MacOS/X")).unwrap();
    std::fs::write(
        app.join("Contents/Info.plist"),
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\"><dict>\
         <key>CFBundleExecutable</key><string>X</string>\
         <key>CFBundleIdentifier</key><string>com.example.x</string>\
         </dict></plist>",
    )
    .unwrap();
    let signed = Command::new("codesign")
        .args(["--force", "-s", "-"])
        .arg(&app)
        .output()
        .unwrap();
    assert!(signed.status.success(), "{signed:?}");

    let ok = app_item(&app);
    let broken = {
        let mut exe = std::fs::read(app.join("Contents/MacOS/X")).unwrap();
        exe.push(0);
        std::fs::write(app.join("Contents/MacOS/X"), exe).unwrap();
        app_item(&app)
    };
    std::fs::remove_dir_all(&dir).unwrap();

    assert_eq!(ok.fields["signature_valid"], "yes");
    assert!(!ok.fields.contains_key("signature_problem"));
    assert_eq!(broken.fields["signature_valid"], "no");
    assert_eq!(
        broken.fields["signature_problem"],
        "main executable failed strict validation"
    );
    assert_eq!(explain(&broken).risk, "high");
}
