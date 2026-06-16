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

# Install ImageMagick if not already available
if ! command -v magick &> /dev/null; then
    echo "Installing ImageMagick (ARM64)..."
    imagemagick_url="https://github.com/ImageMagick/ImageMagick/releases/download/7.1.2-25/ImageMagick-7.1.2-25-portable-Q16-HDRI-arm64.7z"
    archive_path="/tmp/ImageMagick-arm64.7z"
    extract_path="/c/ImageMagick"
    
    curl -L -o "$archive_path" "$imagemagick_url"
    mkdir -p "$extract_path"
    7z x "$archive_path" -o"$extract_path"
    
    # Find magick.exe and add to PATH
    magick_dir=$(find "$extract_path" -name magick.exe -type f | head -1 | xargs dirname)
    if [ -z "$magick_dir" ]; then
        echo "Failed to find magick.exe in extracted archive"
        echo "Contents of $extract_path:"
        find "$extract_path" -type f | head -20
        exit 1
    fi
    
    export PATH="$magick_dir:$PATH"
    export MAGICK_CONFIGURE_PATH="$magick_dir"
    echo "ImageMagick installed at: $magick_dir"
fi

# Cleanup.
function cleanup {
    rm -rf Tango.iconset tango_win_workdir
}
trap cleanup EXIT
cleanup

# Create icon. `resource.rc` (which references icon.ico) is rendered by
# tango's build.rs, which only embeds the resource when icon.ico exists,
# so the icon must be in place before `cargo build` below.
mkdir Tango.iconset
"${MAGICK_EXE:-magick}" tango/src/icon_16.png -resize 16x16 -depth 32 Tango.iconset/icon_16x16.png
"${MAGICK_EXE:-magick}" tango/src/icon.png -resize 32x32 -depth 32 Tango.iconset/icon_32x32.png
"${MAGICK_EXE:-magick}" tango/src/icon.png -resize 128x128 -depth 32 Tango.iconset/icon_128x128.png
"${MAGICK_EXE:-magick}" tango/src/icon.png -resize 256x256 -depth 32 Tango.iconset/icon_256x256.png
"${MAGICK_EXE:-magick}" Tango.iconset/*.png tango/icon.ico
rm -rf Tango.iconset

# Build Windows binaries. MSVC target — statically links the MSVC
# runtime so no mingw DLL bundling is needed.
cargo build --bin tango --profile release-dist --target aarch64-pc-windows-msvc

# Build installer.
mkdir tango_win_workdir
tools/mako_generate.py "$(dirname "${BASH_SOURCE[0]}")/installer.nsi.mako" >tango_win_workdir/installer.nsi

pushd tango_win_workdir

cp ../tango/icon.ico .
cp ../target/aarch64-pc-windows-msvc/release-dist/tango.exe .

chrome_149_url="https://dl.google.com/tag/s/appguid%3D%7B8A69D345-D564-463C-AFF1-A69D9E530F96%7D%26iid%3D%7B9B029E50-463F-4D00-B622-FE96D0D82E97%7D%26browser%3D4%26usagestats%3D0%26appname%3DGoogle%2520Chrome%26needsadmin%3Dtrue%26ap%3Darm64-stable-statsdef_0%26brand%3DGCEA/dl/chrome/install/GoogleChromeStandaloneEnterprise_Arm64.msi"
wget "${chrome_149_url}"
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

ffmpeg_version="8.1.1"
curl -L -o ffmpeg.exe "https://github.com/HCTOrganization/ffmpeg-build/releases/download/ffmpeg-${ffmpeg_version}/ffmpeg-windows-arm64.exe"

makensis installer.nsi
popd

mkdir -p dist
mv tango_win_workdir/installer.exe "dist/tango-aarch64-windows.exe"
rm -rf tango_win_workdir
