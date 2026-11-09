# Driftwatch prior art check

Checked 2026-11-09. Question: does any existing tool diff persistent items on a Mac over time, explain them in plain words, attribute them to the app that made them, and undo them?

## Verdict

**A gap exists, but it is narrower than assumed.** Objective-See already covers two of the pieces we thought were open:

- **KnockKnock 4.1.0 diffs scans.** You save a scan as JSON, then compare it to a new scan; items show as `+` new, `-` removed, `~` modified, across 20 persistence categories. It is manual (no schedule, no stored history), gives no plain-language explanation ("takes no other steps to classify" items), no attribution to the installing app, and no removal. [1]
- **BlockBlock 2.5.2 attributes changes live.** Each alert names the responsible process (name, pid, path, arguments, ancestry) and the item added; Block deletes the item. It needs Endpoint Security and Full Disk Access, keeps a log file but no history view, and only covers "common persistence locations". [2]

What nobody does, as far as this search found:

1. **Automatic history.** A timeline of what changed, when, kept for you without saving JSON files by hand.
2. **Plain-language explanations** of each item ("this is Zoom's updater; it runs hourly as you").
3. **Coverage beyond persistence:** privacy (TCC) grants, certificates, hosts/DNS, SSH keys, firewall and sharing settings, config profiles, apps whose signature changed. KnockKnock and BlockBlock stop at persistence.
4. **Reversible undo** (disable or restore, not just delete).
5. **Weekly digest** for non-experts.

**Strongest differentiator:** automatic history plus plain-language explanation across more than persistence. **Weakest:** live attribution, because BlockBlock does it well and does it with Endpoint Security, which we will not have in v1.

**Biggest risk to the idea:** KnockKnock could add scheduled scans and stored history. Its diff engine already exists; Driftwatch must win on explanation, breadth and undo, not on diffing.

## Feature matrix

`yes` / `no` = stated by the source. `missing` = not verified.

| Tool | Categories | History over time | Plain words | Attributes to app | Undo | Alert / digest | For non-experts | Price |
|---|---|---|---|---|---|---|---|---|
| KnockKnock 4.1.0 [1] | 20 persistence types (launch items, login items, BTM, cron, shell rc, browser ext, system ext, kexts, auth plugins, …) | Manual save + compare only | no | no | no (reveal in Finder) | no | partly | free, source on GitHub |
| BlockBlock 2.5.2 [2] | "common persistence locations" | log file only | no | **yes**, process + ancestry | Block = delete | live alert | partly | free |
| macOS Login Items & Extensions + BTM banner [3][4] | login items, launch agents/daemons from apps | no | no | names developer, not always app [4] | toggle off | live banner | yes | built in |
| `sfltool dumpbtm` [3] | BTM items | no (snapshot) | no | developer/team ID | no | no | no | built in |
| osquery [5] | launchd, startup_items, many tables | yes, differential scheduled queries | no | missing | no | via your pipeline | no | free |
| Jamf Aftermath [6] | IR collection: launch daemons, system ext, shell/browser history, unified logs | no (one-shot collection) | no | no | no | no | no | free |
| Lingon Pro [7] | launchd jobs | missing | no | no | edit/disable | notifies when a task runs | yes | paid |
| CIRCL ALOD [8] | LaunchAgents/Daemons folders | no | no | no | no | Folder Action alert | partly | free, old |
| Santa (North Pole) [9] | binary authorization; has a launch-item monitoring recipe | missing | missing | missing | missing | missing | no | free |
| EtreCheck, CleanMyMac, Kandji, Jamf Protect | | missing | missing | missing | missing | missing | | |

## Notes

- **No good Mac before/after installer diff exists today.** People use `tmutil` snapshots plus a diff tool, or old tools like File Buddy and InstallEase. [10] This supports the "before/after view for installs" feature.
- **Apple's BTM banner is the free baseline** users already see. Its known flaw: it often names the developer, not the app, and can't tell you what the item does. [4] Driftwatch should start from `sfltool dumpbtm` data and fix exactly that.
- **Endpoint Security** is how BlockBlock gets live attribution. v1 polling will attribute by evidence instead (signing team ID, bundle path, plist `Program` path, file timestamps near an install). Mark attribution as "likely" when it's inferred.

## Name check

- **Driftwatch:** taken by an iOS anchor-alarm app on the App Store (id1001235751) and a Shopify inventory tool. [11] Different categories; fine for a DMG outside the App Store, a conflict if we ever ship to the Mac App Store. No GitHub or Homebrew collision found (Homebrew not checked: missing).
- **Aftermath:** taken by Jamf's open-source macOS incident response tool, same domain. Drop it.
- **Residue:** missing (not checked).

## Assumptions and gaps

- EtreCheck, CleanMyMac, Kandji, Jamf Protect and Santa were not verified; cells say missing.
- Lingon's "monitor all tasks in the background" is about tasks running, not folders changing; one summary claimed otherwise and was not confirmed.
- KnockKnock and BlockBlock pages give no release dates.

## Sources

1. https://objective-see.org/products/knockknock.html
2. https://objective-see.org/products/blockblock.html
3. https://support.apple.com/guide/deployment/manage-login-items-background-tasks-mac-depdca572563/web
4. https://mjtsai.com/blog/2022/07/28/ventura-notifies-user-of-new-login-items
5. https://osquery.readthedocs.io/en/latest/deployment/anomaly-detection/
6. https://github.com/jamf/aftermath
7. https://www.peterborgapps.com/lingon/
8. https://www.circl.lu/pub/tr-08
9. https://northpole.security/cookbook/monitor-launch-items.md
10. https://tinyapps.org/blog/202310170700_macos_track_apps.html
11. https://apps.apple.com/us/app/id1001235751
