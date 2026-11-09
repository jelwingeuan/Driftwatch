# Driftwatch v1: design and build plan

## Context
The prior-art check ([docs/prior-art.md](/Users/jelwingeuan/Documents/Claude Code/Driftwatch/docs/prior-art.md), pushed to github.com/jelwingeuan/Driftwatch) found that KnockKnock can diff persistence scans by hand and BlockBlock can attribute changes live. Nobody keeps an automatic history, explains items in plain words, covers more than persistence, or offers reversible undo. Driftwatch v1 fills exactly that gap for **non-technical Mac owners**. Goal beyond the goal: you never wonder whether something on your Mac changed without you knowing.

## Decisions (from the user)
| Topic | Decision |
|---|---|
| Audience | Non-technical owner; raw details behind a "Details" disclosure |
| Coverage | Startup items (launch agents/daemons, login/BTM items, cron, shell rc files), system extensions, privacy (TCC) grants, certificates, hosts + DNS, SSH keys (`authorized_keys`, `~/.ssh`), changed app signatures. Browser extensions + config profiles → v1.1 |
| Shape | Menu bar extra + a window |
| Stack | Rust core (scan, diff, store, explain) + SwiftUI app, via UniFFI |
| Undo | Read-only by default; reversible disable for startup items only (bootout + move plist to `~/Library/Application Support/Driftwatch/Quarantine`, restorable). Never deletes |
| Explanations | Offline: per-category templates + bundled catalog of known apps/team IDs. Nothing leaves the Mac |
| Signing | Ad hoc signed DMG (first-run doc: right-click → Open) |
| macOS | 14 Sonoma+ |
| Features | Timeline, live alerts (new risky items only), before/after install, weekly digest, trusted baseline, export |
| Extras I added (cut any) | Attribution with a confidence label ("added by Zoom — likely"); "Ignore this item" rule; search/filter in timeline; Full Disk Access onboarding screen that explains why and checks it |

## Architecture (smallest thing that works)
```
Driftwatch/
  core/            Rust crate `driftwatch-core`
    src/collect/   one module per category → Vec<Item>
    src/diff.rs    snapshot A vs B → Vec<Change {added|removed|modified}>
    src/store.rs   SQLite (rusqlite): snapshots, items, changes, rules
    src/explain.rs templates + catalog.json → title, sentence, risk
    src/attrib.rs  team ID / bundle path / Program path / mtime near install
    src/undo.rs    disable/restore for launch items
    src/ffi.rs     UniFFI exports
  app/             Xcode project, SwiftUI (MenuBarExtra + Window)
  scripts/build-dmg.sh   cargo build → xcframework → xcodebuild → codesign -s - → hdiutil
```
- **Item** = `{category, key (stable id, e.g. plist path or label), fields (map), signer team ID, hash}`. Diff by key, modified when hash differs.
- **Collectors** read files and run built-in tools only: plist parsing (`plist` crate), `sfltool dumpbtm`, `crontab -l`, `security find-certificate -a`, `codesign -dv`, `/etc/hosts`, `scutil --dns`, `systemextensionsctl list`, TCC.db read-only (needs FDA; mark "missing" without it).
- **Scheduling**: FSEvents (`notify` crate) on watched paths triggers a scan of that category; full rescan every 30 min and at wake. Lives in the app process (launch at login via `SMAppService`), no daemon in v1.
- **Risk**: three levels from simple rules (unsigned / not in catalog / runs as root / persistence + network permission). Labeled as a hint.

## Build order (each step = one commit + push)
1. Cargo workspace, `Item`, `diff.rs` with unit tests.
2. Launch agents/daemons collector + SQLite store; CLI `driftwatch scan` prints the diff. (Verifiable on this Mac.)
3. Remaining collectors, one commit each.
4. Explain + catalog + attribution + risk.
5. Undo (disable/restore) with tests on a temp LaunchAgent.
6. UniFFI bindings; SwiftUI menu bar + timeline window; FDA onboarding.
7. Live alerts, before/after install, baseline, ignore rules, weekly digest, export.
8. `build-dmg.sh` (use the `ship-mac-app` skill), GitHub release.

UI work uses apple-look (HIG review), apple-motion and frontend-design; every step follows ponytail.

## Verification
- `cargo test` for diff, explain, undo, each collector against fixture files.
- Live check per collector: create a test LaunchAgent / hosts entry / cron line, run `driftwatch scan`, confirm it shows as added with a sensible explanation, then remove it and confirm "removed".
- Undo: disable then restore a test agent; `launchctl print gui/$UID/<label>` confirms state each way.
- Ground truth (from the original plan): clean macOS VM, install ~20 popular apps one at a time, compare Driftwatch's changes to `pkgutil` + `sfltool dumpbtm` output. Target: zero missed items. Needs a VM (UTM/Tart), flagged as a later step.
- UI: run the app, screenshot menu bar and window in light/dark, run an apple-look review.
