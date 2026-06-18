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

# openssl-src (pulled in by datachannel-sys's vendored feature) builds
# OpenSSL from source via OpenSSL's Configure perl script, which needs
# modules (Locale::Maketext::Simple, etc.) that Git Bash's minimal MSYS
# perl doesn't ship. Strawberry Perl (preinstalled on the windows-2022
# runner) has them; promote it to the front of PATH.
if [ -d "/c/Strawberry/perl/bin" ]; then
    export PATH="/c/Strawberry/perl/bin:$PATH"
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
# mkdir Trill.iconset
# "${MAGICK_EXE:-magick}" tango/src/icon_16.png -resize 16x16 -depth 32 Trill.iconset/icon_16x16.png
# "${MAGICK_EXE:-magick}" tango/src/icon.png -resize 32x32 -depth 32 Trill.iconset/icon_32x32.png
# "${MAGICK_EXE:-magick}" tango/src/icon.png -resize 128x128 -depth 32 Trill.iconset/icon_128x128.png
# "${MAGICK_EXE:-magick}" tango/src/icon.png -resize 256x256 -depth 32 Trill.iconset/icon_256x256.png
# "${MAGICK_EXE:-magick}" Trill.iconset/*.png tango/icon.ico
# rm -rf Trill.iconset

cp "$(dirname "${BASH_SOURCE[0]}")/trill.ico" tango/icon.ico

# Build Windows binaries. MSVC target — statically links the MSVC
# runtime so no mingw DLL bundling is needed.
cargo build --bin tango --profile release-dist --target aarch64-pc-windows-msvc

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
cp ../target/aarch64-pc-windows-msvc/release-dist/tango.exe trill.exe

chrome_149_url="https://dl.google.com/tag/s/appguid%3D%7B8A69D345-D564-463C-AFF1-A69D9E530F96%7D%26iid%3D%7B9B029E50-463F-4D00-B622-FE96D0D82E97%7D%26browser%3D4%26usagestats%3D0%26appname%3DGoogle%2520Chrome%26needsadmin%3Dtrue%26ap%3Darm64-stable-statsdef_0%26brand%3DGCEA/dl/chrome/install/GoogleChromeStandaloneEnterprise_Arm64.msi"
curl -L -O "${chrome_149_url}"
7z e -aoa GoogleChromeStandaloneEnterprise_Arm64.msi Binary.GoogleChromeInstaller
7z x -aoa Binary.GoogleChromeInstaller
7z e -aoa updater.7z bin/Offline/{a582ca8d-c961-4de4-8491-0d7d2977d020}/{8A69D345-D564-463c-AFF1-A69D9E530F96}/149.0.7827.115_chrome_installer.exe
7z x 149.0.7827.115_chrome_installer.exe
7z e -aoa chrome.7z {Chrome-bin/149.0.7827.115/libEGL.dll,Chrome-bin/149.0.7827.115/libGLESv2.dll}
rm 149.0.7827.115_chrome_installer.exe
rm chrome.7z
rm updater.7z
rm Binary.GoogleChromeInstaller
rm GoogleChromeStandaloneEnterprise_Arm64.msi

ffmpeg_version="8.1.2"
curl -L -o ffmpeg.exe "https://github.com/HCTOrganization/ffmpeg-build/releases/download/ffmpeg-${ffmpeg_version}/ffmpeg-windows-arm64.exe"

makensis installer.nsi
popd

mkdir -p dist
mv trill_win_workdir/installer.exe "dist/trill-aarch64-windows.exe"
rm -rf trill_win_workdir
