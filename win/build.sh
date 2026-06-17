#!/bin/bash
set -euo pipefail

# Under Git Bash on Windows, /usr/bin/link.exe (an MSYS coreutil, not
# the MSVC linker) precedes MSVC's link.exe on PATH and rustc ends up
# invoking the wrong one. Promote MSVC's bin dir (located via cl.exe,
# which msvc-dev-cmd has put on PATH) to the front.
if command -v cl.exe >/dev/null 2>&1; then
    msvc_bin=$(dirname "$(command -v cl.exe)")
    export PATH="$msvc_bin:$PATH"
fi

# Cleanup.
function cleanup {
    rm -rf Trill.iconset trill_win_workdir
}
trap cleanup EXIT
cleanup

# Create icon. `resource.rc` (which references icon.ico) is rendered by
# tango's build.rs, which only embeds the resource when icon.ico exists,
# so the icon must be in place before `cargo build` below.
mkdir Trill.iconset
magick tango/src/icon_16.png -resize 16x16 -depth 32 Trill.iconset/icon_16x16.png
magick tango/src/icon.png -resize 32x32 -depth 32 Trill.iconset/icon_32x32.png
magick tango/src/icon.png -resize 128x128 -depth 32 Trill.iconset/icon_128x128.png
magick tango/src/icon.png -resize 256x256 -depth 32 Trill.iconset/icon_256x256.png
magick Trill.iconset/*.png tango/icon.ico
rm -rf Trill.iconset

# Build Windows binaries. MSVC target — statically links the MSVC
# runtime so no mingw DLL bundling is needed.
cargo build --bin tango --profile release-dist --target x86_64-pc-windows-msvc

# Download and install Nsis7z plugin before building installer
nsis7z_archive_url="https://nsis.sourceforge.io/mediawiki/images/6/69/Nsis7z_19.00.7z"
nsis_plugins_dir=""

# Try to find NSIS installation
if [ -d "${PROGRAMFILES}/NSIS/Plugins/x86-unicode" ]; then
    nsis_plugins_dir="${PROGRAMFILES}/NSIS/Plugins/x86-unicode"
elif [ -d "$(cygpath 'C:\Program Files (x86)')/NSIS/Plugins/x86-unicode" ]; then
    nsis_plugins_dir="$(cygpath 'C:\Program Files (x86)')/NSIS/Plugins/x86-unicode"
elif [ -d "/c/Program Files (x86)/NSIS/Plugins/x86-unicode" ]; then
    nsis_plugins_dir="/c/Program Files (x86)/NSIS/Plugins/x86-unicode"
fi

if [ -z "$nsis_plugins_dir" ]; then
    echo "Warning: NSIS plugins directory not found. Creating default location."
    nsis_plugins_dir="/c/Program Files (x86)/NSIS/Plugins/x86-unicode"
    mkdir -p "$nsis_plugins_dir"
fi

# Download and extract Nsis7z plugin
mkdir -p nsis7z_temp
curl -L -o nsis7z_temp/nsis7z.7z "${nsis7z_archive_url}"
7z x -onsis7z_temp nsis7z_temp/nsis7z.7z "Plugins/x86-unicode/nsis7z.dll"
cp nsis7z_temp/Plugins/x86-unicode/nsis7z.dll "$nsis_plugins_dir/"
rm -rf nsis7z_temp

# Build installer.
mkdir trill_win_workdir
tools/mako_generate.py "$(dirname "${BASH_SOURCE[0]}")/installer.nsi.mako" >trill_win_workdir/installer.nsi

pushd trill_win_workdir

cp ../tango/icon.ico .
cp ../target/x86_64-pc-windows-msvc/release-dist/tango.exe trill.exe

angle_zip_url="https://github.com/google/gfbuild-angle/releases/download/github%2Fgoogle%2Fgfbuild-angle%2Ff810e998993290f049bbdad4fae975e4867100ad/gfbuild-angle-f810e998993290f049bbdad4fae975e4867100ad-Windows_x64_Release.zip"
curl -L -o angle.zip "${angle_zip_url}"
unzip -o -j angle.zip "lib/libEGL.dll" "lib/libGLESv2.dll" -d .
rm angle.zip

ffmpeg_version="8.1.1"
curl -L -o ffmpeg.exe "https://github.com/HCTOrganization/ffmpeg-build/releases/download/ffmpeg-${ffmpeg_version}/ffmpeg-windows-x86_64.exe"

makensis installer.nsi
popd

mkdir -p dist
mv trill_win_workdir/installer.exe "dist/trill-x86_64-windows.exe"
rm -rf trill_win_workdir
