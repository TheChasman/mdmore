#!/usr/bin/env bash
# Build FrogRust natively and install it as ~/Applications/FrogRust.app.
#
# FrogRust is a terminal app. The bundle is an AppleScript applet so it can
# receive Finder "Open With" / drag-and-drop file events (a shell-script
# executable can't). Each file opens in a new Terminal window. Launched on its
# own, it asks for a Markdown file. The raw binary is also at
#   ~/Applications/FrogRust.app/Contents/MacOS/frogrust
# for use from a shell.
#
# Usage: scripts/deploy-mac.sh            (installs to ~/Applications)
#        DEST=/some/dir scripts/deploy-mac.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${DEST:-${HOME}/Applications}"
APP_NAME="FrogRust"
BUNDLE_ID="com.thechasman.frogrust"
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
mkdir -p "${BUILD}"

# Applet: `on open` handles Open With / drops, `on run` handles a plain launch.
# The shell work lives in a bash helper so it doesn't depend on /bin/sh's printf.
osacompile -o "${APP}" - <<'APPLESCRIPT' || die "osacompile failed"
on run
	try
		set f to choose file with prompt "Open a Markdown file in FrogRust" of type {"net.daringfireball.markdown", "public.plain-text"}
	on error number -128
		return
	end try
	openInFrogRust(f)
end run

on open theFiles
	repeat with f in theFiles
		openInFrogRust(f)
	end repeat
end open

on openInFrogRust(f)
	set helper to (POSIX path of (path to me)) & "Contents/Resources/open-in-terminal"
	do shell script quoted form of helper & " " & quoted form of (POSIX path of f)
end openInFrogRust
APPLESCRIPT

cp "${ROOT}/target/release/frogrust" "${APP}/Contents/MacOS/frogrust"

# Helper: run frogrust on the file in a new Terminal window.
# It opens a throwaway .terminal settings file whose command *replaces* the
# shell (RunCommandAsShell), so the user's zsh startup (prompts, secret managers)
# never runs. Opening a .terminal file doesn't import a profile, and needs no
# Automation permission. Terminal splits CommandString on spaces without
# honouring quotes, so it points at a space-free temp script that holds the
# properly quoted command and deletes itself.
cat > "${APP}/Contents/Resources/open-in-terminal" <<'HELPER'
#!/bin/bash
set -e
file="$1"
bin="$(cd "$(dirname "$0")/../MacOS" && pwd)/frogrust"
tmpdir="${TMPDIR:-/tmp}"
run="$(mktemp "${tmpdir%/}/frogrust.XXXXXX")"
printf '#!/bin/bash\nrm -f "$0"\ncd %q && exec %q %q\n' "$(dirname "${file}")" "${bin}" "${file}" > "${run}"
chmod +x "${run}"

# Start from the user's default Terminal profile so the window looks familiar.
term="${run}.terminal"
default="$(defaults read com.apple.Terminal "Default Window Settings" 2>/dev/null || true)"
if [[ -z "${default}" || "${default}" == *.* ]] \
    || ! defaults export com.apple.Terminal - 2>/dev/null \
        | plutil -extract "Window Settings.${default}" xml1 -o "${term}" - 2>/dev/null; then
    printf '<?xml version="1.0" encoding="UTF-8"?>\n<plist version="1.0"><dict/></plist>\n' > "${term}"
fi
plutil -replace name -string FrogRust "${term}"
plutil -replace type -string "Window Settings" "${term}"
plutil -replace CommandString -string "${run}" "${term}"
plutil -replace RunCommandAsShell -bool YES "${term}"
plutil -replace shellExitAction -integer 1 "${term}"   # close only on a clean exit

/usr/bin/open -a Terminal "${term}"
# Terminal reads the settings at open; tidy up shortly after.
( sleep 15; rm -f "${term}" ) >/dev/null 2>&1 &
HELPER
chmod +x "${APP}/Contents/Resources/open-in-terminal"

# Icon: master PNG → iconset → icns. Drop the applet's own icon assets.
iconset="${BUILD}/${APP_NAME}.iconset"
rm -rf "${iconset}" && mkdir -p "${iconset}"
for size in 16 32 128 256 512; do
    sips -z "${size}" "${size}" "${ICON_SRC}" --out "${iconset}/icon_${size}x${size}.png" >/dev/null
    sips -z "$((size * 2))" "$((size * 2))" "${ICON_SRC}" --out "${iconset}/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "${iconset}" -o "${APP}/Contents/Resources/${APP_NAME}.icns"
rm -rf "${iconset}" "${APP}"/Contents/Resources/{applet,droplet}.icns "${APP}/Contents/Resources/Assets.car"

# Info.plist: identity, icon, version, and Markdown as a viewable document type
# (this is what puts FrogRust in Finder's "Open With" list).
plist="${APP}/Contents/Info.plist"
plutil -remove CFBundleIconName "${plist}" >/dev/null 2>&1 || true
for kv in "CFBundleName=${APP_NAME}" "CFBundleDisplayName=${APP_NAME}" \
          "CFBundleIdentifier=${BUNDLE_ID}" "CFBundleIconFile=${APP_NAME}" \
          "CFBundleShortVersionString=${version}" "CFBundleVersion=${version}"; do
    plutil -replace "${kv%%=*}" -string "${kv#*=}" "${plist}"
done
plutil -replace CFBundleDocumentTypes -json '[{
    "CFBundleTypeName": "Markdown document",
    "CFBundleTypeRole": "Viewer",
    "LSHandlerRank": "Alternate",
    "LSItemContentTypes": ["net.daringfireball.markdown"]
}]' "${plist}"
plutil -replace UTImportedTypeDeclarations -json '[{
    "UTTypeIdentifier": "net.daringfireball.markdown",
    "UTTypeDescription": "Markdown document",
    "UTTypeConformsTo": ["public.plain-text"],
    "UTTypeTagSpecification": {"public.filename-extension": ["md", "markdown"]}
}]' "${plist}"
plutil -lint "${plist}" >/dev/null || die "Info.plist invalid"

# Strip provenance/quarantine xattrs, then ad-hoc sign the whole bundle.
xattr -cr "${APP}"
codesign --force --deep --sign - "${APP}" >/dev/null 2>&1 || die "codesign failed"
codesign --verify --deep --strict "${APP}" || die "codesign verify failed"

printf '→ installing to %s\n' "${DEST}"
mkdir -p "${DEST}"
rm -rf "${DEST:?}/${APP_NAME}.app"
ditto "${APP}" "${DEST}/${APP_NAME}.app"
# Register with LaunchServices so the icon and Open With entry show up.
touch "${DEST}/${APP_NAME}.app"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "${DEST}/${APP_NAME}.app" 2>/dev/null || true

printf '✓ %s\n' "${DEST}/${APP_NAME}.app"
"${DEST}/${APP_NAME}.app/Contents/MacOS/frogrust" --version
