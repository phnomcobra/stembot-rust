#!/usr/bin/bash
set -e

PACKAGE_NAME="stembot-rust"
VERSION="1.0.0"
ARCH="amd64"
DEB_NAME="${PACKAGE_NAME}_${VERSION}_${ARCH}.deb"
DIST_DIR="$(dirname "$0")/../dist"
BUILD_DIR="$(mktemp -d)"

# Ensure dist directory exists
mkdir -p "$DIST_DIR"

# Remove any existing deb file
rm -f "$DIST_DIR"/*.deb

# Build release binaries
cargo build --release

# Set up deb package directory structure
mkdir -p "$BUILD_DIR/DEBIAN"
mkdir -p "$BUILD_DIR/usr/bin"

# Copy binaries
cp target/release/agt-server "$BUILD_DIR/usr/bin/"
cp target/release/agt-configure "$BUILD_DIR/usr/bin/"
cp target/release/agt-control "$BUILD_DIR/usr/bin/"

# Write control file
cat > "$BUILD_DIR/DEBIAN/control" <<EOF
Package: $PACKAGE_NAME
Version: $VERSION
Architecture: $ARCH
Maintainer: Justin Dierking <phnomcobra@gmail.com>
Depends: ca-certificates, libssl3, libsqlite3-0, openssl
Description: Stembot Rust Agent
EOF

# Build the deb
dpkg-deb --build "$BUILD_DIR" "$DIST_DIR/$DEB_NAME"

# Clean up
rm -rf "$BUILD_DIR"

echo "Built: $DIST_DIR/$DEB_NAME"
