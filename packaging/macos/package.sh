#!/usr/bin/env bash
# Build and package Polykrom for macOS:
#
#   $DIST/solidcraft-<version>-macos-<arch>.dmg        SolidCraft.app (with polykrom-cli inside) on a
#                                                      drag-to-Applications DMG
#   $DIST/solidcraft-cli-<version>-macos-<arch>.zip    the headless CLI, polykrom-cli (and its
#                                                      solidcraft-cli and solvecraft-cli aliases) with the licence files
#
# Usage: packaging/macos/package.sh [--arch universal|aarch64|x86_64] [--skip-build]
#
# The app is ad-hoc signed (not notarized): the first launch needs right-click > Open.
# Adapted from PhotoCraft's packaging (storytold/photocraft, Apache-2.0; see ATTRIBUTION.md).
set -euo pipefail
# shellcheck source=../env.sh
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
# The same reverse-DNS id as the Linux APP_ID.
MACOS_BUNDLE_ID=ai.storyteller.solidcraft

ARCH=universal
SKIP_BUILD=0
while [ $# -gt 0 ]; do
  case "$1" in
    --arch) ARCH="$2"; shift 2 ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    -h | --help) sed -n '2,10p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
case "$ARCH" in
  universal) TARGETS=(aarch64-apple-darwin x86_64-apple-darwin) ;;
  aarch64) TARGETS=(aarch64-apple-darwin) ;;
  x86_64) TARGETS=(x86_64-apple-darwin) ;;
  *) echo "unknown --arch $ARCH" >&2; exit 2 ;;
esac

# Keep in sync with LSMinimumSystemVersion in Info.plist.in.
export MACOSX_DEPLOYMENT_TARGET=11.0
SHORT_VERSION="${VERSION%%-*}"
WORK="$CARGO_TARGET_DIR/macos-package"
APP="$WORK/SolidCraft.app"
DMG="$DIST/solidcraft-$VERSION-macos-$ARCH.dmg"
CLI_ZIP="$DIST/solidcraft-cli-$VERSION-macos-$ARCH.zip"

echo "==> Polykrom $VERSION for macOS ($ARCH)"
if [ "$SKIP_BUILD" = 0 ]; then
  args=()
  for t in "${TARGETS[@]}"; do args+=(--target "$t"); done
  (cd "$ROOT" && cargo build --release --locked -p polykrom -p polykrom-cli "${args[@]}")
fi

rm -rf "$WORK"
mkdir -p "$WORK/bin"
for bin in polykrom polykrom-cli solidcraft-cli solvecraft-cli; do
  inputs=()
  for t in "${TARGETS[@]}"; do inputs+=("$CARGO_TARGET_DIR/$t/release/$bin"); done
  lipo -create -output "$WORK/bin/$bin" "${inputs[@]}"
  lipo -info "$WORK/bin/$bin"
done

# ---- SolidCraft.app ----------------------------------------------------------------------------
echo "==> assembling $APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$WORK/bin/polykrom" "$APP/Contents/MacOS/Polykrom"
# polykrom-cli next to the app, as every other package has it: the agent chat starts it as the
# agent's MCP server (`polykrom-cli mcp --connect`) and looks for it beside the running program.
# solidcraft-cli beside it keeps MCP registrations that name the old program working
# (docs/compatibility.md).
cp "$WORK/bin/polykrom-cli" "$APP/Contents/MacOS/polykrom-cli"
cp "$WORK/bin/solidcraft-cli" "$APP/Contents/MacOS/solidcraft-cli"
# Apple-compiled layered icon and older-macOS fallback (regeneration: assets/README.md).
# Keep compiled resources so packaging also works on runners with older Xcode versions.
cp "$ROOT/assets/icons/app/macos/exports/Assets.car" "$APP/Contents/Resources/Assets.car"
cp "$ROOT/assets/icons/app/macos/exports/icon.icns" "$APP/Contents/Resources/Polykrom.icns"
mkdir -p "$APP/Contents/Resources/Licenses"
copy_docs "$APP/Contents/Resources/Licenses"
sed -e "s/@VERSION@/$VERSION/g" -e "s/@SHORT_VERSION@/$SHORT_VERSION/g" "$ROOT/assets/platforms/macos/Info.plist.in" >"$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist"
printf 'APPL????' >"$APP/Contents/PkgInfo"
# Ad-hoc signature over the whole bundle (Apple silicon refuses unsigned code). Nested code is
# signed first: the bundle signature refuses an unsigned helper in Contents/MacOS.
codesign --force --sign - --timestamp=none --identifier "$MACOS_BUNDLE_ID-cli" "$APP/Contents/MacOS/polykrom-cli"
codesign --force --sign - --timestamp=none --identifier "$MACOS_BUNDLE_ID-cli-alias" "$APP/Contents/MacOS/solidcraft-cli"
codesign --force --sign - --timestamp=none "$APP"
codesign --verify --strict --deep --verbose=2 "$APP"
for cli in polykrom-cli solidcraft-cli; do
  test -x "$APP/Contents/MacOS/$cli" || { echo "$cli is missing from $APP" >&2; exit 1; }
done

# ---- DMG ---------------------------------------------------------------------------------------
echo "==> building $DMG"
STAGE="$WORK/dmg"
mkdir -p "$STAGE"
ditto "$APP" "$STAGE/SolidCraft.app"
ln -s /Applications "$STAGE/Applications"
cp "$ROOT/assets/platforms/macos/README.txt" "$STAGE/README.txt"
rm -f "$DMG" "$WORK/raw.dmg"
# makehybrid + convert builds the image without attaching a device, unlike `create -srcfolder`,
# which is flaky on CI runners ("Resource busy").
hdiutil makehybrid -hfs -hfs-volume-name "Polykrom" -hfs-openfolder "$STAGE" -o "$WORK/raw.dmg" "$STAGE"
hdiutil convert "$WORK/raw.dmg" -format UDZO -imagekey zlib-level=9 -o "$DMG"
rm -f "$WORK/raw.dmg"

# Check the image as a user gets it: mount it and look inside.
MNT="$(mktemp -d)"
hdiutil attach -readonly -nobrowse -mountpoint "$MNT" "$DMG"
ls -la "$MNT" "$MNT/SolidCraft.app/Contents" "$MNT/SolidCraft.app/Contents/MacOS" "$MNT/SolidCraft.app/Contents/Resources"
test -L "$MNT/Applications" && test "$(readlink "$MNT/Applications")" = /Applications
plutil -lint "$MNT/SolidCraft.app/Contents/Info.plist"
# Check the names Finder/LaunchServices actually use, and the bundle identity.
for key in CFBundleName CFBundleDisplayName CFBundleExecutable CFBundleIconFile CFBundleIconName; do
  test "$(/usr/libexec/PlistBuddy -c "Print :$key" "$MNT/SolidCraft.app/Contents/Info.plist")" = Polykrom
done
test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$MNT/SolidCraft.app/Contents/Info.plist")" = "$MACOS_BUNDLE_ID"
test -s "$MNT/SolidCraft.app/Contents/Resources/Polykrom.icns"
cmp "$ROOT/assets/icons/app/macos/exports/icon.icns" "$MNT/SolidCraft.app/Contents/Resources/Polykrom.icns"
cmp "$ROOT/assets/icons/app/macos/exports/Assets.car" "$MNT/SolidCraft.app/Contents/Resources/Assets.car"
cmp "$ROOT/assets/platforms/macos/README.txt" "$MNT/README.txt"
case "$ARCH" in universal) WANT=(arm64 x86_64) ;; aarch64) WANT=(arm64) ;; *) WANT=(x86_64) ;; esac
for arch in "${WANT[@]}"; do
  lipo "$MNT/SolidCraft.app/Contents/MacOS/Polykrom" -verify_arch "$arch"
done
test -s "$MNT/SolidCraft.app/Contents/Resources/Licenses/THIRD-PARTY-LICENSES.txt"
# Not --strict: the mounted HFS volume adds Finder info, which strict verification rejects.
codesign --verify --deep --verbose=2 "$MNT/SolidCraft.app"
"$MNT/SolidCraft.app/Contents/MacOS/Polykrom" --version
hdiutil detach "$MNT"

# ---- CLI ---------------------------------------------------------------------------------------
echo "==> building $CLI_ZIP"
CLI_DIR="$WORK/solidcraft-cli-$VERSION-macos-$ARCH"
mkdir -p "$CLI_DIR"
cp "$WORK/bin/polykrom-cli" "$CLI_DIR/"
codesign --force --sign - --timestamp=none --identifier "$MACOS_BUNDLE_ID-cli" "$CLI_DIR/polykrom-cli"
# Compatibility shims (docs/compatibility.md): the earlier CLI names forward to polykrom-cli.
for alias in solidcraft-cli solvecraft-cli; do
  cp "$WORK/bin/$alias" "$CLI_DIR/"
  codesign --force --sign - --timestamp=none --identifier "$MACOS_BUNDLE_ID-$alias-alias" "$CLI_DIR/$alias"
done
copy_docs "$CLI_DIR"
rm -f "$CLI_ZIP"
ditto -c -k --keepParent "$CLI_DIR" "$CLI_ZIP"

"$WORK/bin/polykrom-cli" --version
"$WORK/bin/solidcraft-cli" --version
echo "==> done"
ls -lh "$DMG" "$CLI_ZIP"
