#!/usr/bin/env bash
# Build a universal macOS DMG for Manor, ad-hoc signed with hardened runtime.
#
# Output: target/universal-apple-darwin/release/bundle/dmg/Manor_<version>_universal.dmg
#
# The DMG is NOT notarized — it's signed ad-hoc so macOS Gatekeeper will show
# "unidentified developer" on first launch. Users right-click → Open to bypass.
# See docs/INSTALL.md for the end-user install instructions.
#
# To produce a truly clean, notarized DMG, enrol in the Apple Developer Program,
# add a Developer ID Application cert to Keychain, and set:
#     APPLE_ID=...
#     APPLE_PASSWORD=<app-specific>
#     APPLE_TEAM_ID=...
# then change signingIdentity in tauri.conf.json from "-" to the cert common name.

set -euo pipefail

cd "$(dirname "$0")/.."

# Ensure both macOS Rust targets are installed so the universal lipo succeeds.
for t in aarch64-apple-darwin x86_64-apple-darwin; do
  if ! rustup target list --installed | grep -q "^${t}$"; then
    echo "Adding Rust target ${t}..."
    rustup target add "$t"
  fi
done

cd apps/desktop
pnpm tauri build --target universal-apple-darwin

DMG_DIR="../../target/universal-apple-darwin/release/bundle/dmg"
DMG="$(ls -1 "$DMG_DIR"/*.dmg 2>/dev/null | head -n 1)"

if [[ -z "$DMG" ]]; then
  echo "error: no DMG produced under $DMG_DIR" >&2
  exit 1
fi

echo
echo "Built: $DMG"
ls -lh "$DMG" | awk '{print "Size:  " $5}'

# Mount briefly to verify the signature inside — catches broken builds early.
MOUNT=$(mktemp -d)
hdiutil attach "$DMG" -mountpoint "$MOUNT" -nobrowse -quiet
trap 'hdiutil detach "$MOUNT" -quiet -force >/dev/null 2>&1 || true' EXIT

APP="$MOUNT/Manor.app"
echo
echo "Signature:"
codesign --display --verbose=1 "$APP" 2>&1 | grep -E "^(Signature|Format|Identifier|Runtime)"
echo
echo "Verification:"
codesign --verify --deep --strict "$APP" && echo "  ✓ signed, bundle intact"

hdiutil detach "$MOUNT" -quiet
trap - EXIT
