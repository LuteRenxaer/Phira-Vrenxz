#!/bin/bash
# =============================================================================
# Phira-Vrenxz iOS IPA Build Script (macOS only)
# =============================================================================
# Usage:
#   ./scripts/build_ios.sh [release|debug]
#
# Prerequisites:
#   - Xcode 15+ with iOS SDK
#   - Rust (rustup) with iOS targets:
#       rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
#   - xcodegen: brew install xcodegen
#
# Output:
#   Phira-Vrenxz-ios.ipa in the project root
# =============================================================================

set -euo pipefail

# ---- Configuration ----
CONFIG="${1:-release}"
CONFIG_CAP="$(tr '[:lower:]' '[:upper:]' <<< "${CONFIG:0:1}")${CONFIG:1}"
APP_NAME="Phira-Vrenxz"
BUNDLE_ID="com.teamflos.PhiraVrenxz"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
XCODE_DIR="$PROJECT_ROOT/Phira-Vrenxz/xcode"
DERIVED_DATA="$XCODE_DIR/build/DerivedData"

echo "========================================"
echo "  Phira-Vrenxz iOS Build"
echo "  Configuration: $CONFIG_CAP"
echo "  Project Root: $PROJECT_ROOT"
echo "========================================"

# ---- Check platform ----
if [[ "$(uname)" != "Darwin" ]]; then
    echo "ERROR: This script must be run on macOS."
    echo "On Windows, use the GitHub Actions workflow (.github/workflows/ios.yml)."
    exit 1
fi

# ---- Check tools ----
check_tool() {
    if ! command -v "$1" &>/dev/null; then
        echo "ERROR: $1 not found. $2"
        exit 1
    fi
}

check_tool xcodebuild "Install Xcode from the App Store"
check_tool xcodegen "Run: brew install xcodegen"
check_tool cargo "Install Rust: https://rustup.rs"

# ---- Ensure iOS targets ----
echo ""
echo "[1/5] Checking Rust iOS targets..."
INSTALLED_TARGETS=$(rustup target list --installed 2>/dev/null || true)
for target in aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios; do
    if ! grep -q "^$target$" <<<"$INSTALLED_TARGETS"; then
        echo "  Installing $target..."
        rustup target add "$target"
    fi
done
echo "  iOS targets ready."

# ---- Generate Xcode project ----
echo ""
echo "[2/5] Generating Xcode project with xcodegen..."
cd "$XCODE_DIR"
xcodegen generate
# Force object version to 56 (Xcode 14+) for Xcode 15 compatibility
sed -i '' 's/objectVersion = [0-9]*;/objectVersion = 56;/' Phira-Vrenxz.xcodeproj/project.pbxproj
echo "  Project generated: $XCODE_DIR/Phira-Vrenxz.xcodeproj"

# ---- Build Rust static library (device) ----
echo ""
echo "[3/5] Building Rust static library for aarch64-apple-ios..."
cd "$PROJECT_ROOT"
export IPHONEOS_DEPLOYMENT_TARGET=14.0
if [ "$CONFIG" = "release" ]; then
    cargo build --release --target aarch64-apple-ios --features video -p Phira-Vrenxz
else
    cargo build --target aarch64-apple-ios --features video -p Phira-Vrenxz
fi
echo "  Static library: target/aarch64-apple-ios/$CONFIG/libphira_vrenxz.a"

# ---- Build iOS app ----
echo ""
echo "[4/5] Building iOS app with xcodebuild..."
cd "$XCODE_DIR"
rm -rf build/DerivedData

xcodebuild build \
    -project Phira-Vrenxz.xcodeproj \
    -scheme Phira-Vrenxz \
    -configuration "$CONFIG_CAP" \
    -sdk iphoneos \
    -derivedDataPath "$DERIVED_DATA" \
    CODE_SIGNING_ALLOWED=NO \
    CODE_SIGN_IDENTITY="" \
    CODE_SIGNING_REQUIRED=NO \
    ONLY_ACTIVE_ARCH=NO \
    ARCHS=arm64 \
    2>&1 | tee build.log

# Find the .app
APP_PATH=$(find "$DERIVED_DATA" -name "${APP_NAME}.app" -type d | head -1)
if [ -z "$APP_PATH" ]; then
    echo "ERROR: Could not find ${APP_NAME}.app in DerivedData"
    echo "Check build.log for errors."
    exit 1
fi
echo "  App built: $APP_PATH"

# ---- Package IPA ----
echo ""
echo "[5/5] Packaging IPA..."
IPA_DIR="$PROJECT_ROOT/ipa_build"
rm -rf "$IPA_DIR"
mkdir -p "$IPA_DIR/Payload"
cp -R "$APP_PATH" "$IPA_DIR/Payload/"

# Remove code signature for unsigned IPA
if [ -d "$IPA_DIR/Payload/${APP_NAME}.app/_CodeSignature" ]; then
    rm -rf "$IPA_DIR/Payload/${APP_NAME}.app/_CodeSignature"
fi

IPA_NAME="${APP_NAME}-ios-${CONFIG}.ipa"
cd "$IPA_DIR"
zip -r "../$IPA_NAME" Payload/
cd "$PROJECT_ROOT"
rm -rf "$IPA_DIR"

echo ""
echo "========================================"
echo "  BUILD SUCCESS"
echo "  IPA: $PROJECT_ROOT/$IPA_NAME"
echo "  Size: $(du -h "$IPA_NAME" | cut -f1)"
echo "========================================"
echo ""
echo "To install on a device:"
echo "  - Use Sideloadly / AltStore (free Apple ID, 7-day cert)"
echo "  - Use TrollStore (permanent, no signing needed)"
echo "  - Or sign with your Apple Developer certificate"
