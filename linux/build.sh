#!/usr/bin/env bash
set -euo pipefail

cleanup() {
    rm -rf trill_linux_workdir
}
trap cleanup EXIT
cleanup

# Grab a copy of appimagetool.
wget https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage
chmod a+x appimagetool-x86_64.AppImage

# Build Linux binaries.
target_arch="x86_64"
cargo build --bin trill --features gamesupport-all --target="${target_arch}-unknown-linux-gnu" --profile release-dist

# Assemble AppImage stuff.
mkdir -p "trill_linux_workdir/${target_arch}/bin"
cp trill/src/icon.png trill_linux_workdir/trill.png
cp linux/AppRun trill_linux_workdir/AppRun
cp linux/trill.desktop trill_linux_workdir/trill.desktop
cp "target/${target_arch}-unknown-linux-gnu/release-dist/trill" "trill_linux_workdir/${target_arch}/bin/trill"

# Split the DWARF (release-dist builds with debug = 1) out of the
# shipped binary into a sidecar release asset — the ELF analogue of
# the Windows .pdb — so users don't download line tables with every
# update but crash-log module+offset frames still resolve offline.
mkdir -p dist
objcopy --only-keep-debug "trill_linux_workdir/${target_arch}/bin/trill" "dist/trill-${target_arch}-linux.debug"
objcopy --strip-debug --add-gnu-debuglink="dist/trill-${target_arch}-linux.debug" "trill_linux_workdir/${target_arch}/bin/trill"

# Bundle ffmpeg.
ffmpeg_version="8.1.2"

wget "https://github.com/HCTOrganization/ffmpeg-build/releases/download/ffmpeg-${ffmpeg_version}/ffmpeg-linux-x86_64" -O "trill_linux_workdir/${target_arch}/bin/ffmpeg"
chmod a+x "trill_linux_workdir/${target_arch}/bin/ffmpeg"

# Build AppImage. appimagetool defaults to gzip squashfs; xz roughly
# halves the download for a modest first-access decompression cost
# (blocks land in the page cache after that), and the AppImageKit
# runtime decompresses it natively.
./appimagetool-x86_64.AppImage --comp xz trill_linux_workdir "dist/trill-${target_arch}-linux.AppImage"
rm -rf trill_linux_workdir
