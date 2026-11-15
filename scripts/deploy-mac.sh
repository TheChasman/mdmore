#!/usr/bin/env bash
# Build FrogRust natively and install it as ~/Applications/FrogRust.app.
#
# FrogRust is a terminal app, so the bundle's launcher asks for a Markdown file
# and opens it in Terminal. The raw binary is also at
#   ~/Applications/FrogRust.app/Contents/MacOS/frogrust
# for use from a shell.
#
# Usage: scripts/deploy-mac.sh            (installs to ~/Applications)
#        DEST=/some/dir scripts/deploy-mac.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${DEST:-${HOME}/Applications}"
APP_NAME="FrogRust"
ICON_SRC="${ROOT}/artwork/icon/RustFrog.png"
BUILD="${ROOT}/target/macos"
APP="${BUILD}/${APP_NAME}.app"

die() { printf '✗ %s\n' "$1" >&2; exit 1; }

# Native builds only — never Rosetta/cross (account rule).
host="$(rustc -vV | sed -n 's/^host: //p')"
case "$(uname -m)" in
    arm64)  [[ "${host}" == aarch64-apple-darwin ]] || die "rustc host ${host} is not native arm64" ;;
    x86_64) [[ "${host}" == x86_64-apple-darwin ]]  || die "rustc host ${host} is not native x86_64" ;;
    *) die "unsupported machine $(uname -m)" ;;
esac
[[ -f "${ICON_SRC}" ]] || die "icon not found: ${ICON_SRC}"

version="$(sed -n 's/^version = "\(.*\)"/\1/p' "${ROOT}/Cargo.toml" | head -1)"

printf '→ cargo build --release (%s)\n' "${host}"
cargo build --release --manifest-path "${ROOT}/Cargo.toml"

printf '→ assembling %s.app %s\n' "${APP_NAME}" "${version}"
rm -rf "${APP}"
mkdir -p "${APP}/Contents/MacOS" "${APP}/Contents/Resources"
cp "${ROOT}/target/release/frogrust" "${APP}/Contents/MacOS/frogrust"

# Icon: master PNG → iconset → icns
iconset="${BUILD}/${APP_NAME}.iconset"
rm -rf "${iconset}" && mkdir -p "${iconset}"
for size in 16 32 128 256 512; do
    sips -z "${size}" "${size}" "${ICON_SRC}" --out "${iconset}/icon_${size}x${size}.png" >/dev/null
    sips -z "$((size * 2))" "$((size * 2))" "${ICON_SRC}" --out "${iconset}/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "${iconset}" -o "${APP}/Contents/Resources/${APP_NAME}.icns"
rm -rf "${iconset}"

# Launcher: pick a file, then run frogrust on it in a new Terminal window.
# A .command file avoids needing Automation permission to script Terminal.
cat > "${APP}/Contents/MacOS/launcher" <<'LAUNCHER'
#!/bin/bash
here="$(cd "$(dirname "$0")" && pwd)"
file="$(/usr/bin/osascript -e 'POSIX path of (choose file with prompt "Open a Markdown file in FrogRust" of type {"net.daringfireball.markdown", "public.plain-text"})' 2>/dev/null)" || exit 0
tmp="$(mktemp -t frogrust)" && cmd="${tmp}.command" && mv "${tmp}" "${cmd}"
printf '#!/bin/bash\nrm -f "$0"\ncd %q && exec %q %q\n' "$(dirname "${file}")" "${here}/frogrust" "${file}" > "${cmd}"
chmod +x "${cmd}"
exec /usr/bin/open -a Terminal "${cmd}"
LAUNCHER
chmod +x "${APP}/Contents/MacOS/launcher"

cat > "${APP}/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>${APP_NAME}</string>
    <key>CFBundleDisplayName</key><string>${APP_NAME}</string>
    <key>CFBundleIdentifier</key><string>com.thechasman.frogrust</string>
    <key>CFBundleExecutable</key><string>launcher</string>
    <key>CFBundleIconFile</key><string>${APP_NAME}</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>${version}</string>
    <key>CFBundleVersion</key><string>${version}</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

# Strip provenance/quarantine xattrs, then ad-hoc sign the whole bundle.
xattr -cr "${APP}"
codesign --force --deep --sign - "${APP}" >/dev/null 2>&1 || die "codesign failed"
codesign --verify --deep --strict "${APP}" || die "codesign verify failed"

printf '→ installing to %s\n' "${DEST}"
mkdir -p "${DEST}"
rm -rf "${DEST:?}/${APP_NAME}.app"
ditto "${APP}" "${DEST}/${APP_NAME}.app"
# Nudge Finder/LaunchServices to pick up the new icon.
touch "${DEST}/${APP_NAME}.app"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "${DEST}/${APP_NAME}.app" 2>/dev/null || true

printf '✓ %s\n' "${DEST}/${APP_NAME}.app"
"${DEST}/${APP_NAME}.app/Contents/MacOS/frogrust" --version
