# Installing Manor on macOS

Manor is free, open source, and distributed as a universal macOS DMG (works on
both Apple Silicon and Intel Macs).

The DMG is **ad-hoc signed, not notarized** — notarization requires a paid
Apple Developer membership and the app stays free. macOS Gatekeeper will show
a "developer cannot be verified" warning the first time you open it. The steps
below walk through it.

## First-time install

1. **Download** `Manor_<version>_universal.dmg` from the [Releases page](../../releases).
2. **Open** the DMG and drag `Manor.app` into the `Applications` folder.
3. **Open Applications**, find Manor, **right-click → Open**.
    - macOS shows: *"macOS cannot verify the developer of 'Manor'. Are you sure you want to open it?"*
    - Click **Open**.
4. Manor launches. macOS will not prompt again — the app is trusted from now on.

## "Manor is damaged and can't be opened"

You shouldn't see this with a freshly downloaded DMG from the Releases page,
but it can happen if:

- The DMG was extracted from a zip on a case-insensitive filesystem
- An antivirus quarantined the bundle mid-extract
- You're running an unusually strict Gatekeeper profile

One-line fix in Terminal:

```bash
xattr -cr /Applications/Manor.app
```

This strips the quarantine flag macOS attached at download time. Then launch
Manor normally.

## Uninstall

- Drag `Manor.app` from `Applications` to the Bin.
- Manor's data lives at `~/Library/Application Support/com.hanamorix.manor/` —
  remove that folder if you want a clean slate.
- Remove entries from `~/Library/Preferences/com.hanamorix.manor.plist` if present.

## Why not notarize?

Apple notarization requires a $99/year Developer Program membership. Manor is
free and open-source; the maintainer has opted against an ongoing cost for that
single UX polish. The one-time right-click-Open is the tradeoff.

If you'd prefer a notarized build, the signing path is a two-line change in
`apps/desktop/src-tauri/tauri.conf.json` (swap `signingIdentity: "-"` for your
Developer ID common name) plus setting `APPLE_ID` / `APPLE_PASSWORD` /
`APPLE_TEAM_ID` before running `./scripts/release-mac.sh`.

## Building from source

See the [README](../README.md) for dev setup. To produce your own DMG:

```bash
./scripts/release-mac.sh
```

Output lands at `target/universal-apple-darwin/release/bundle/dmg/`.
