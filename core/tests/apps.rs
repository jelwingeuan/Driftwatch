use driftwatch::collect::apps::{app_item, only_detritus, parse_codesign};
use driftwatch::explain::explain;
use std::path::{Path, PathBuf};
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

/// An ad hoc signed X.app in a fresh temp dir named after `test` (remove the dir after).
fn signed_app(test: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("driftwatch-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
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
    (dir, app)
}

fn append_to_executable(app: &Path) {
    let mut exe = std::fs::read(app.join("Contents/MacOS/X")).unwrap();
    exe.push(0);
    std::fs::write(app.join("Contents/MacOS/X"), exe).unwrap();
}

/// Finder info on the bundle folder, the "detritus" `codesign --strict` rejects.
fn add_finder_info(app: &Path) {
    let set = Command::new("xattr")
        .args(["-wx", "com.apple.FinderInfo"])
        .arg("0000000000000000400000000000000000000000000000000000000000000000")
        .arg(app)
        .output()
        .unwrap();
    assert!(set.status.success(), "{set:?}");
}

#[test]
fn app_whose_signature_breaks_after_signing_fails_the_check() {
    let (dir, app) = signed_app("broken");
    let ok = app_item(&app);
    let broken = {
        append_to_executable(&app);
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

#[test]
fn detritus_alone_is_only_a_note_but_never_hides_a_real_problem() {
    let (dir, app) = signed_app("detritus");
    add_finder_info(&app);
    let detritus = app_item(&app);
    // Strict reports only the detritus here; the broken executable is behind it.
    append_to_executable(&app);
    let strict = Command::new("codesign")
        .args(["--verify", "--strict"])
        .arg(&app)
        .output()
        .unwrap();
    let hidden = app_item(&app);
    std::fs::remove_dir_all(&dir).unwrap();

    assert_eq!(detritus.fields["signature_valid"], "yes");
    assert!(!detritus.fields.contains_key("signature_problem"));
    assert_eq!(
        detritus.fields["signature_note"],
        "Has extra file metadata macOS ignores (strict check: resource fork, Finder information, or similar detritus not allowed)"
    );
    assert_eq!(explain(&detritus).risk, "low");

    assert!(only_detritus(&String::from_utf8_lossy(&strict.stderr)));
    assert_eq!(hidden.fields["signature_valid"], "no");
    assert_eq!(
        hidden.fields["signature_problem"],
        "main executable failed strict validation"
    );
    assert!(!hidden.fields.contains_key("signature_note"));
    assert_eq!(explain(&hidden).risk, "high");
}

#[test]
fn only_detritus_means_detritus_is_the_one_complaint() {
    let detritus =
        "/Applications/A.app: resource fork, Finder information, or similar detritus not allowed";
    assert!(only_detritus(&format!("{detritus}\n")));
    assert!(!only_detritus(&format!(
        "{detritus}\n/Applications/A.app: a sealed resource is missing or invalid\n"
    )));
    assert!(!only_detritus(
        "/Applications/A.app: a sealed resource is missing or invalid\n"
    ));
    assert!(!only_detritus(
        "/Applications/A.app: invalid destination for symbolic link in bundle\n"
    ));
    assert!(!only_detritus(""));
}
