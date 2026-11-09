<p align="center">
  <img src="assets/logo/icon-1024.png" width="128" alt="Driftwatch icon">
</p>

<h1 align="center">Driftwatch</h1>

<p align="center">See what changed on your Mac without you knowing.</p>

Apps quietly add things to your Mac: helpers that start at login, permissions to your camera or files, certificates, network settings. Driftwatch scans these places, keeps a history, and shows you what was added, changed or removed since the last scan, so nothing changes silently.

It runs in the menu bar and in a timeline window. Everything stays on your Mac: no account, no network calls.

## What it watches

| Area | What it looks at |
|---|---|
| Startup items | Launch agents and daemons, login and background items, cron jobs, shell startup files |
| System extensions | Installed system extensions |
| Privacy permissions | Which apps can use your camera, microphone, files, screen and more (needs Full Disk Access) |
| Certificates | Certificates in your login and System keychains and their trust settings |
| Network | `/etc/hosts`, DNS resolvers |
| SSH | `authorized_keys`, SSH config, public keys, and which private keys exist (their contents are never read) |
| Apps | Code signatures of apps in `/Applications` and `~/Applications` |

Driftwatch only reads. It never deletes or changes anything on its own.

## Status

Early, v0.1. The scanner, history and the menu bar app work. Still to come:

- Plain-language explanations and risk hints for each item
- Turning off a startup item, with a way to bring it back
- Alerts for new risky items, before/after an install, a trusted baseline, a weekly digest
- A downloadable DMG

## Build from source

Needs macOS 14 or later, Rust (`cargo`) and Xcode command line tools (`swift`).

```bash
git clone https://github.com/jelwingeuan/Driftwatch.git
cd Driftwatch
scripts/build-app.sh
open build/Driftwatch.app
```

The script builds the Rust scanner and the SwiftUI app, puts both in `build/Driftwatch.app`, and signs it ad hoc.

### Full Disk Access

Privacy permissions live in a protected database. To see them, add Driftwatch in System Settings › Privacy & Security › Full Disk Access. Without it, everything else still works and that area shows as unreadable.

## Command line

The scanner also works on its own:

```bash
cargo run --release -- scan          # scan and print what changed since the last scan
cargo run --release -- scan --json   # the same, as JSON
cargo run --release -- history --json --limit 20
```

History is stored in `~/Library/Application Support/Driftwatch/driftwatch.db`.

## How it's built

- `core/`: Rust. One collector per area, a diff of each scan against the last, and a SQLite store.
- `app/`: SwiftUI menu bar app and window. It runs the bundled scanner and reads its JSON.
- `docs/`: the [prior-art check](docs/prior-art.md) and the [v1 plan](docs/plan.md).

## License

Not licensed yet.
