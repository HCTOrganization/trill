#!/bin/bash
set -euo pipefail

# Cleanup.
function cleanup {
	rm -rf trill_linux_workdir
}
trap cleanup EXIT
cleanup

# Grab a copy of appimagetool.
wget -c https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-i686.AppImage
chmod a+x appimagetool-i686.AppImage

# Build Linux binaries.
target_arch="i686"
cargo build --bin tango --target="${target_arch}-unknown-linux-gnu" --profile release-dist

# Assemble AppImage stuff.
mkdir -p "trill_linux_workdir/${target_arch}/bin"
cp tango/src/icon.png trill_linux_workdir/trill.png
cp linux/AppRun trill_linux_workdir/AppRun
cp linux/tango.desktop trill_linux_workdir/trill.desktop
cp "target/${target_arch}-unknown-linux-gnu/release-dist/tango" "trill_linux_workdir/${target_arch}/bin/trill"

# Bundle ffmpeg.
ffmpeg_version="6.0"

wget -c "https://github.com/eugeneware/ffmpeg-static/releases/download/b${ffmpeg_version}/ffmpeg-linux-ia32" -O "trill_linux_workdir/${target_arch}/bin/ffmpeg"
chmod a+x "trill_linux_workdir/${target_arch}/bin/ffmpeg"

# Build AppImage.
mkdir -p dist
# Workaround for running 32bit OS on 64bit kernel
cd trill_linux_workdir
ln -s i686 i386
ln -s i686 x86_64
ln -s i686 amd64
cd ..
./appimagetool-i686.AppImage trill_linux_workdir "dist/trill-${target_arch}-linux.AppImage"
rm -rf trill_linux_workdir
