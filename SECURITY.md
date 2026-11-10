# Security policy

## Reporting a problem

Please report security problems privately, not in a public issue: open the **Security** tab of this repository and click **Report a vulnerability**. That sends it through GitHub's private vulnerability reporting.

## What's in scope

- The scanner (`core/`, the `driftwatch` command line tool)
- The app (`app/`)
- The release pipeline (`.github/workflows/`, `scripts/`, and the DMGs on the Releases page)

## Supported versions

Only the latest release gets fixes.

## Good to know

- Driftwatch only reads. It never deletes or changes anything, and it makes no network calls.
- The app is signed ad hoc and not notarized by Apple, so macOS warns you the first time you open it.
- Each release comes with a SHA-256 checksum file and a GitHub build attestation, so you can check that the DMG is the one this repository built. See [Install](README.md#install).
