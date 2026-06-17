#!/bin/bash
set -euo pipefail

# Cleanup.
function cleanup {
	rm -rf trill_linux_workdir
}
trap cleanup EXIT
cleanup

# Grab a copy of appimagetool.
wget https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage
chmod a+x appimagetool-x86_64.AppImage

# Build Linux binaries.
target_arch="x86_64"
cargo build --bin tango --target="${target_arch}-unknown-linux-gnu" --profile release-dist

# Assemble AppImage stuff.
mkdir -p "trill_linux_workdir/${target_arch}/bin"
cp tango/src/icon.png trill_linux_workdir/trill.png
cp linux/AppRun trill_linux_workdir/AppRun
cp linux/tango.desktop trill_linux_workdir/trill.desktop
cp "target/${target_arch}-unknown-linux-gnu/release-dist/tango" "trill_linux_workdir/${target_arch}/bin/trill"

# Bundle ffmpeg.
ffmpeg_version="8.1.1"

wget "https://github.com/HCTOrganization/ffmpeg-build/releases/download/ffmpeg-${ffmpeg_version}/ffmpeg-linux-x86_64" -O "trill_linux_workdir/${target_arch}/bin/ffmpeg"
chmod a+x "trill_linux_workdir/${target_arch}/bin/ffmpeg"

# Build AppImage.
mkdir -p dist
./appimagetool-x86_64.AppImage trill_linux_workdir "dist/trill-${target_arch}-linux.AppImage"
rm -rf trill_linux_workdir
