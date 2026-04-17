#!/usr/bin/env bash
# Grep exits 1 on no matches; wrap each call so pipefail doesn't abort.
set -uo pipefail

count() { { grep "$@" 2>/dev/null || true; } | wc -l | tr -d ' '; }

echo "=== Legacy tokens remaining ==="
echo

echo "-- --imessage-* refs --"
count -rn "imessage-blue\|imessage-green\|imessage-red" apps/desktop/src \
  --include="*.tsx" --include="*.ts" --include="*.css"

echo
echo "-- Phase-5 dark hex --"
count -rn "#1a1a2e\|#16213e\|#3a2a12" apps/desktop/src \
  --include="*.tsx" --include="*.ts"

echo
echo "-- Pure #fff / #000 in inline styles --"
count -rn '"#fff"\|"#000"\|"#FFF"\|"#000000"\|"#ffffff"\|"white"\|"black"' \
  apps/desktop/src --include="*.tsx"

echo
echo "-- Cool rgba neutrals --"
count -rn "rgba(0,\s*0,\s*0" apps/desktop/src \
  --include="*.tsx" --include="*.ts"

echo
echo "-- Nunito references --"
count -rn "Nunito\|@fontsource/nunito" apps/desktop/src \
  --include="*.tsx" --include="*.ts" --include="*.css"

echo
echo "-- border-left > 1px --"
count -rn "borderLeft.*[2-9]px solid\|border-left.*[2-9]px solid" apps/desktop/src \
  --include="*.tsx" --include="*.ts" --include="*.css"

echo
echo "-- backdrop-filter decorative --"
count -rn "backdropFilter\|backdrop-filter" apps/desktop/src \
  --include="*.tsx" --include="*.ts" --include="*.css"
