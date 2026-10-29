#!/usr/bin/env bash
set -euo pipefail

# F9 experiment: keep Tauri's own executable/resources and use the OS WebView.
# Never remove libraries from an existing checkout or overwrite the source bundle.
source_dir="$(realpath target/release/bundle/appimage/AntiVector.AppDir)"
test -f "$source_dir/usr/bin/av-desktop"
work="$(mktemp -d "$(pwd)/target/lean-appimage.XXXXXX")"
appdir="$work/AntiVector.AppDir"
mkdir -p "$appdir/usr/bin" "$appdir/usr/lib/AntiVector" "$appdir/runtime-notices"
cp "$source_dir/usr/bin/av-desktop" "$appdir/usr/bin/av-desktop"
cp -a "$source_dir/usr/lib/AntiVector/notices" "$appdir/usr/lib/AntiVector/notices"
cp -L "$source_dir/AntiVector.desktop" "$appdir/AntiVector.desktop"
cp -L "$source_dir/AntiVector.png" "$appdir/AntiVector.png"
ln -s AntiVector.png "$appdir/.DirIcon"
cat > "$appdir/AppRun" <<'ENTRY'
#!/bin/sh
set -eu
exec "${APPDIR:?}/usr/bin/av-desktop" "$@"
ENTRY
chmod +x "$appdir/AppRun" "$appdir/usr/bin/av-desktop"
ldd "$appdir/usr/bin/av-desktop" | tee "$work/system-dependencies.txt"
if grep -q 'not found' "$work/system-dependencies.txt"; then exit 1; fi

fetch() { curl --fail --location --retry 3 "$1" --output "$2"; }
# Official tool/runtime assets. A moving upstream asset must fail the hash check.
fetch https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage "$work/appimagetool"
echo "95cbe7cce9717fce90c484e34052ee7c7f1d7635b33c12525b4776826a7d29b6  $work/appimagetool" | sha256sum -c -
fetch https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-x86_64 "$work/runtime"
echo "617aa77314d57c8b8e4fae9c3f0b35af7b30d5529cb69e98f64f92510ed98a46  $work/runtime" | sha256sum -c -
chmod +x "$work/appimagetool"

# Corresponding runtime source, build scripts, libfuse source and applied patches.
# This bootstrap is independent of the AntiVector executable. No GPL utilities
# from these upstream source archives are linked into AntiVector.
runtime_commit=fedfad51acc2860b2916a1eef32ba4a500ff31ef
fetch "https://github.com/AppImage/type2-runtime/archive/$runtime_commit.tar.gz" "$appdir/runtime-notices/type2-runtime-source.tar.gz"
fetch https://github.com/libfuse/libfuse/releases/download/fuse-3.15.0/fuse-3.15.0.tar.xz "$appdir/runtime-notices/fuse-3.15.0.tar.xz"
echo "70589cfd5e1cff7ccd6ac91c86c01be340b227285c5e200baa284e401eea2ca0  $appdir/runtime-notices/fuse-3.15.0.tar.xz" | sha256sum -c -
fetch https://github.com/vasi/squashfuse/archive/0.5.2.tar.gz "$appdir/runtime-notices/squashfuse-0.5.2.tar.gz"
echo "db0238c5981dabbd80ee09ae15387f390091668ca060a7bc38047912491443d3  $appdir/runtime-notices/squashfuse-0.5.2.tar.gz" | sha256sum -c -
fetch "https://raw.githubusercontent.com/AppImage/type2-runtime/$runtime_commit/LICENSE" "$appdir/runtime-notices/LICENSE-runtime.txt"
fetch 'https://git.musl-libc.org/cgit/musl/plain/COPYRIGHT?h=v1.2.5' "$appdir/runtime-notices/LICENSE-musl.txt"
fetch https://raw.githubusercontent.com/microsoft/mimalloc/v2.1.7/LICENSE "$appdir/runtime-notices/LICENSE-mimalloc.txt"
fetch https://raw.githubusercontent.com/facebook/zstd/v1.5.6/LICENSE "$appdir/runtime-notices/LICENSE-zstd.txt"
fetch https://raw.githubusercontent.com/madler/zlib/v1.3.1/LICENSE "$appdir/runtime-notices/LICENSE-zlib.txt"
cat > "$appdir/runtime-notices/README.txt" <<'NOTICE'
AntiVector AppImage bootstrap
Runtime: AppImage/type2-runtime fedfad51acc2860b2916a1eef32ba4a500ff31ef.
Runtime asset SHA256:617aa77314d57c8b8e4fae9c3f0b35af7b30d5529cb69e98f64f92510ed98a46.
The runtime is a separate MIT program containing libfuse3 (LGPL-2.1),
squashfuse (BSD), musl (MIT), mimalloc (MIT), zstd (BSD) and zlib (Zlib).
See the accompanying upstream LICENSE, source archives and their notices.
The complete runtime source includes the build scripts and libfuse patch.
To modify/relink libfuse, unpack the source archives, apply the included patch
and follow type2-runtime's BUILD.md / scripts/docker build instructions.
Use appimagetool --runtime-file <your rebuilt runtime> to replace the bootstrap.
No dependency licenses or recipient rights are restricted by AntiVector.
The application uses GTK3 and WebKitGTK4.1 provided by the operating system;
these libraries are not copied into this AppImage. Tested baseline:Ubuntu24.04.
NOTICE

output="$(pwd)/target/release/bundle/appimage/AntiVector_0.1.0_amd64-system-webview.AppImage"
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$work/appimagetool" --runtime-file "$work/runtime" --comp zstd "$appdir" "$output"
chmod +x "$output"
# Keep the oversized reference outside the distribution artifact; never delete it.
mv "$(dirname "$source_dir")/AntiVector_0.1.0_amd64.AppImage" "$work/reference-full.AppImage"
cp "$work/system-dependencies.txt" target/release/bundle/appimage/system-dependencies.txt
sha256sum "$appdir"/runtime-notices/* > target/release/bundle/appimage/runtime-sources.sha256
printf '%s\n' "$output"
