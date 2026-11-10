use driftwatch::collect::apps::{app_item, only_detritus, parse_codesign, REUSE_LIMIT};
use driftwatch::diff::diff;
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
    let ok = app_item(&app, None);
    let broken = {
        append_to_executable(&app);
        app_item(&app, None)
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
    let detritus = app_item(&app, None);
    // Strict reports only the detritus here; the broken executable is behind it.
    append_to_executable(&app);
    let strict = Command::new("codesign")
        .args(["--verify", "--strict"])
        .arg(&app)
        .output()
        .unwrap();
    let hidden = app_item(&app, None);
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

#[test]
fn an_unchanged_app_reuses_its_last_check_for_up_to_a_day() {
    let (dir, app) = signed_app("reuse");
    let first = app_item(&app, None);
    let again = app_item(&app, Some(&first));
    let mut day_old = first.clone();
    day_old.signature_check.as_mut().unwrap().checked_at -= REUSE_LIMIT;
    let rechecked = app_item(&app, Some(&day_old));

    // A file added to the bundle breaks the seal but none of the files the reuse
    // key covers, so only a fresh check notices it.
    std::fs::create_dir_all(app.join("Contents/Resources")).unwrap();
    std::fs::write(app.join("Contents/Resources/extra.txt"), "x").unwrap();
    let reused = app_item(&app, Some(&first));
    let expired = app_item(&app, Some(&day_old));
    let mut unknown = first.clone();
    unknown
        .fields
        .insert("signature_valid".into(), "unknown".into());
    let not_reused = app_item(&app, Some(&unknown));

    // Same modify time, new change time: the key changes, so the app is checked.
    let plist = std::fs::File::options()
        .write(true)
        .open(app.join("Contents/Info.plist"))
        .unwrap();
    plist
        .set_modified(plist.metadata().unwrap().modified().unwrap())
        .unwrap();
    let touched = app_item(&app, Some(&first));
    std::fs::remove_dir_all(&dir).unwrap();

    assert_eq!(first.fields["signature_valid"], "yes");
    assert!(first.signature_check.is_some());
    // Reused as it was: same verdict, same check time, no change in the diff.
    assert_eq!(again, first);
    assert!(diff(std::slice::from_ref(&first), &[again]).is_empty());
    // A day on, checked again: a new check time, still no change in the diff.
    let at = |i: &driftwatch::Item| i.signature_check.as_ref().unwrap().checked_at;
    assert!(at(&rechecked) >= at(&first));
    assert!(diff(&[day_old], &[rechecked]).is_empty());

    assert_eq!(reused.fields["signature_valid"], "yes");
    for checked in [&expired, &not_reused, &touched] {
        assert_eq!(checked.fields["signature_valid"], "no", "{checked:?}");
        assert!(at(checked) >= at(&first));
    }
}
