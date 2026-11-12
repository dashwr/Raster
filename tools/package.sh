#!/usr/bin/env bash
# Build no cezanne e pacote pacman local: dist/raster-<versão>-1-x86_64.pkg.tar.zst
# pkgver = versão do Cargo + .r<commits>.g<hash> (formato VCS do pacman).
set -euo pipefail
cd "$(dirname "$0")/.."
ver=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
rev=$(git rev-parse --short=7 HEAD)
n=$(git rev-list --count HEAD)
dirty=$([ -n "$(git status --porcelain -- src runtime/dsh agents Cargo.toml Cargo.lock build.rs)" ] && echo .dirty || true)
pkgver="${ver}.r${n}.g${rev}${dirty/./_}"
rsync -az --delete --exclude target --exclude .git --exclude dist ./ cezanne:build/raster/
ssh cezanne "bash -lc 'cd build/raster && RASTER_REV=${rev}${dirty} cargo build --release --locked 2>&1 | tail -2'"
mkdir -p target/release dist
scp -q cezanne:build/raster/target/release/raster target/release/raster.new
mv target/release/raster.new target/release/raster
export RASTER_PKGVER="$pkgver" PKGDEST="$PWD/dist" BUILDDIR="$(mktemp -d)"
(cd packaging && makepkg -f --nodeps >/dev/null)
ls -1 dist/raster-${pkgver}-*.pkg.tar.zst
