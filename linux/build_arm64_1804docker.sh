#!/bin/bash
set -euo pipefail

# Build the arm64 Linux AppImage inside an Ubuntu 18.04 container.
#
# Why: AppImages link against the glibc of the build host. Building on a newer
# distro (e.g. the ubuntu-22.04-arm CI runner, glibc 2.35) produces a binary
# that refuses to start on older systems. Ubuntu 18.04 ships glibc 2.27, so the
# result runs on a much wider range of machines.
#
# Usage:
#   ./linux/build_arm64_1804docker.sh
#
# The script is self-relaunching: invoked on the host it starts the container
# and re-executes itself inside it (TRILL_IN_DOCKER=1), where the real build
# runs. Requires Docker on an arm64 host (the GitHub ubuntu-*-arm runners work).

target_arch="aarch64"
ffmpeg_version="8.1.2"

# Pinned toolchain versions. 18.04's apt packages are too old:
#   - cmake 3.10, but libdatachannel's libsrtp needs cmake >= 3.21.
#   - protoc 3.0, but signaling.proto uses proto3 `optional` (needs >= 3.15).
# So we fetch modern standalone builds of both instead of using apt.
cmake_version="3.30.5"
protoc_version="28.3"
ubuntu_image="ubuntu:18.04"

# ---------------------------------------------------------------------------
# Host side: launch the container and re-run this same script inside it.
# ---------------------------------------------------------------------------
if [ "${TRILL_IN_DOCKER:-}" != "1" ]; then
	repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

	# Persist the cargo registry/git caches across runs by mounting the host's
	# CARGO_HOME into the container as root's CARGO_HOME. The compile output
	# (target/) already persists because it lives under the mounted repo. Cache
	# these host paths in CI to avoid re-downloading every crate each run.
	host_cargo_home="${CARGO_HOME:-${HOME}/.cargo}"
	mkdir -p "${host_cargo_home}"

	exec docker run --rm \
		--platform linux/arm64 \
		-e TRILL_IN_DOCKER=1 \
		-e DEBIAN_FRONTEND=noninteractive \
		-e CMAKE_POLICY_VERSION_MINIMUM=3.5 \
		-v "${repo_root}:/src" \
		-v "${host_cargo_home}:/root/.cargo" \
		-w /src \
		"${ubuntu_image}" \
		bash linux/build_arm64_1804docker.sh
fi

# ---------------------------------------------------------------------------
# Container side: everything below runs inside Ubuntu 18.04.
# ---------------------------------------------------------------------------

# Cleanup.
function cleanup {
	rm -rf trill_linux_workdir
}
trap cleanup EXIT
cleanup

# System build dependencies. These mirror the librust-*-dev packages used by
# the CI on 22.04, mapped to the underlying dev packages that exist on 18.04:
#   librust-atk-dev          -> libatk1.0-dev
#   librust-gdk-pixbuf-dev   -> libgdk-pixbuf2.0-dev
#   librust-gdk-sys-dev      -> libgtk-3-dev (provides gdk-3.0)
#   librust-pango-dev        -> libpango1.0-dev
#   alsa                     -> libasound2-dev
apt-get update -y
apt-get install -y --no-install-recommends \
	build-essential \
	clang \
	ca-certificates \
	curl \
	desktop-file-utils \
	file \
	git \
	libasound2-dev \
	libatk1.0-dev \
	libgdk-pixbuf2.0-dev \
	libgtk-3-dev \
	libnss3 \
	libpango1.0-dev \
	libsdl2-dev \
	libssl-dev \
	pkg-config \
	unzip \
	wget \
	xz-utils

# Modern cmake (apt's 3.10 is too old for libsrtp's `cmake >= 3.21`).
wget -q "https://github.com/Kitware/CMake/releases/download/v${cmake_version}/cmake-${cmake_version}-linux-aarch64.tar.gz" -O /tmp/cmake.tar.gz
tar xzf /tmp/cmake.tar.gz -C /opt
export PATH="/opt/cmake-${cmake_version}-linux-aarch64/bin:${PATH}"

# Modern protoc (apt's 3.0 can't parse proto3 `optional`).
wget -q "https://github.com/protocolbuffers/protobuf/releases/download/v${protoc_version}/protoc-${protoc_version}-linux-aarch_64.zip" -O /tmp/protoc.zip
unzip -q /tmp/protoc.zip -d /opt/protoc
export PATH="/opt/protoc/bin:${PATH}"
export PROTOC="/opt/protoc/bin/protoc"

# Rust toolchain (18.04 has no usable rustc in apt). CARGO_HOME (/root/.cargo)
# may be a restored cache mount, so only run the installer when rustup isn't
# already there; re-running rustup-init over an existing install errors out.
export CARGO_HOME="${HOME}/.cargo"
if [ ! -x "${CARGO_HOME}/bin/rustup" ]; then
	curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
		| sh -s -- -y --default-toolchain stable --profile minimal --no-modify-path
fi
# shellcheck disable=SC1090
source "${CARGO_HOME}/env"
rustup toolchain install stable --profile minimal
rustup target add "${target_arch}-unknown-linux-gnu"

# Grab a copy of appimagetool.
wget https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-aarch64.AppImage
chmod a+x appimagetool-aarch64.AppImage

# Build Linux binaries.
cargo build --bin tango --target="${target_arch}-unknown-linux-gnu" --profile release-dist

# Assemble AppImage stuff.
mkdir -p "trill_linux_workdir/${target_arch}/bin"
cp tango/src/icon.png trill_linux_workdir/trill.png
cp linux/AppRun trill_linux_workdir/AppRun
cp linux/tango.desktop trill_linux_workdir/trill.desktop
cp "target/${target_arch}-unknown-linux-gnu/release-dist/tango" "trill_linux_workdir/${target_arch}/bin/trill"

# Bundle ffmpeg.
wget "https://github.com/HCTOrganization/ffmpeg-build/releases/download/ffmpeg-${ffmpeg_version}/ffmpeg-linux-arm64" -O "trill_linux_workdir/${target_arch}/bin/ffmpeg"
chmod a+x "trill_linux_workdir/${target_arch}/bin/ffmpeg"

# Build AppImage. FUSE isn't available in an unprivileged container, so tell
# appimagetool to extract and run itself instead of mounting via FUSE.
export APPIMAGE_EXTRACT_AND_RUN=1
mkdir -p dist
./appimagetool-aarch64.AppImage trill_linux_workdir "dist/trill-${target_arch}-linux.AppImage"
rm -rf trill_linux_workdir
