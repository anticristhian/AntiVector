#!/usr/bin/env bash
set -euo pipefail
image="$(realpath target/release/bundle/appimage/AntiVector_0.1.0_amd64-system-webview.AppImage)"
work="$(mktemp -d "$(pwd)/target/smoke-appimage.XXXXXX")"
set +e
AV_STARTUP_MARKER_PATH="$work/ready.txt" APPIMAGE_EXTRACT_AND_RUN=1 \
    timeout 30s dbus-run-session -- xvfb-run --auto-servernum "$image" > "$work/output.txt" 2>&1
result=$?
set -e
cat "$work/output.txt"
cp "$work/output.txt" target/release/bundle/appimage/smoke-output.txt
test "$result" -eq 124
grep -qx 'embedded-webview-page-finished' "$work/ready.txt"
cp "$work/ready.txt" target/release/bundle/appimage/smoke-ready.txt
printf 'AppImage started its embedded WebView under Xvfb/D-Bus; exit124 is the intentional30s timeout.\n'
