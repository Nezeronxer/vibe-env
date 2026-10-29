#!/bin/sh
# One-command installer: downloads the latest release from GitHub Releases, verifies its SHA-256
# checksum and installs it. No telemetry, nothing but the release download is contacted.
#   sh install.sh            install the latest release
#   sh install.sh --dry-run  show what would be done (still reads the release list, installs nothing)
set -eu

# --- the only places that name the product and the repository ---------------------------------
APP_NAME="Nightshift"                 # ASSUMPTION: working name; must equal productName in app/tauri.conf.json
REPO="Nezeronxer/vibe-env"
# -----------------------------------------------------------------------------------------------

DRY_RUN=0
[ "${1:-}" = "--dry-run" ] && DRY_RUN=1

say()  { printf '%s\n' "$*"; }
step() { printf '\n[%s/4] %s\n' "$1" "$2"; }
die()  { printf 'error: %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

case "$REPO" in
  *"<"*|*">"*|"") die "REPO is still a placeholder in this script. Set it to the real GitHub owner/name, or install by hand from the Releases page." ;;
esac
have curl || die "curl is required"
# https only, TLS 1.2+
curl_s() { curl --proto '=https' --tlsv1.2 "$@"; }

OS=$(uname -s)
MACHINE=$(uname -m)
TMP=$(mktemp -d "${TMPDIR:-/tmp}/nightshift-install.XXXXXX")
trap 'rm -rf "$TMP"' EXIT INT TERM

# --- 1. find the latest release ---------------------------------------------------------------
step 1 "Looking up the latest release of $REPO"
API="https://api.github.com/repos/$REPO/releases/latest"
curl_s -fsSL -H 'Accept: application/vnd.github+json' "$API" -o "$TMP/release.json" || die "could not read $API (is there a published, non-draft release?)"
URLS=$(sed -n 's/.*"browser_download_url": *"\([^"]*\)".*/\1/p' "$TMP/release.json")
[ -n "$URLS" ] || die "the latest release has no assets"

# pick_asset <regex>: first release asset URL whose file name matches
pick_asset() { printf '%s\n' "$URLS" | grep -E "/${APP_NAME}[_-][^/]*$1\$" | head -n 1 || true; }

# --- decide what to install -------------------------------------------------------------------
KIND=""; ASSET=""
case "$OS" in
  Darwin)
    case "$MACHINE" in arm64|aarch64) ARCH=aarch64 ;; x86_64) ARCH=x64 ;; *) die "unsupported CPU: $MACHINE" ;; esac
    KIND=dmg; ASSET=$(pick_asset "_$ARCH\\.dmg")
    ;;
  Linux)
    case "$MACHINE" in x86_64|amd64) : ;; *) die "no Linux build for $MACHINE yet (only x86_64). Build from source: see README." ;; esac
    ID=""; ID_LIKE=""
    # shellcheck disable=SC1091
    [ -r /etc/os-release ] && . /etc/os-release
    FAMILY=" $ID $ID_LIKE "
    case "$FAMILY" in
      *" debian "*|*" ubuntu "*)                         KIND=deb ;;
      *" fedora "*|*" rhel "*|*" suse "*|*" opensuse "*) KIND=rpm ;;
      *" arch "*)                                        KIND=arch ;;
      *)                                                 KIND=appimage ;;
    esac
    case "$KIND" in
      deb) ASSET=$(pick_asset "_amd64\\.deb") ;;
      rpm) ASSET=$(pick_asset "\\.x86_64\\.rpm") ;;
      arch)
        say "Arch family detected. Install from the AUR package or build the PKGBUILD in app/installer/aur/ (see README)."
        say "Falling back to the AppImage, which runs on any distribution."
        KIND=appimage; ASSET=$(pick_asset "_amd64\\.AppImage") ;;
      appimage) ASSET=$(pick_asset "_amd64\\.AppImage") ;;
    esac
    ;;
  *) die "unsupported OS: $OS (macOS and Linux only)" ;;
esac
[ -n "$ASSET" ] || die "no matching file ($KIND, $MACHINE) in the latest release"
FILE=${ASSET##*/}
say "Found $FILE"
if [ "$DRY_RUN" = 1 ]; then say "(dry run) would download, verify and install $FILE as $KIND"; exit 0; fi

# --- 2. download and verify the checksum ------------------------------------------------------
step 2 "Downloading $FILE and verifying its checksum"
curl_s -fL --progress-bar "$ASSET" -o "$TMP/$FILE" || die "download failed"
SUMS_URL=$(printf '%s\n' "$URLS" | grep -E '/SHA256SUMS$' | head -n 1 || true)
if [ -z "$SUMS_URL" ] && [ "${NIGHTSHIFT_SKIP_VERIFY:-0}" != 1 ]; then
  die "the release has no SHA256SUMS file; refusing to install an unverified file (set NIGHTSHIFT_SKIP_VERIFY=1 to override at your own risk)"
fi
if [ -z "$SUMS_URL" ]; then
  say "WARNING: NIGHTSHIFT_SKIP_VERIFY=1 - installing WITHOUT any checksum verification." >&2
fi
if [ -n "$SUMS_URL" ]; then
  curl_s -fsSL "$SUMS_URL" -o "$TMP/SHA256SUMS" || die "could not download SHA256SUMS"
  # exact match on the second field (the file name), optionally with the binary-mode "*" prefix
  WANT=$(awk -v f="$FILE" '$2 == f || $2 == "*" f { print $1; exit }' "$TMP/SHA256SUMS")
  [ -n "$WANT" ] || die "$FILE is not listed in SHA256SUMS"
  if have sha256sum; then GOT=$(sha256sum "$TMP/$FILE" | awk '{print $1}'); else GOT=$(shasum -a 256 "$TMP/$FILE" | awk '{print $1}'); fi
  [ "$WANT" = "$GOT" ] || die "checksum mismatch for $FILE - not installing"
  say "Checksum OK ($GOT)"
fi

# --- 3. install -------------------------------------------------------------------------------
step 3 "Installing"
case "$KIND" in
  dmg)
    MNT="$TMP/mnt"; mkdir -p "$MNT"
    hdiutil attach -nobrowse -readonly -quiet -mountpoint "$MNT" "$TMP/$FILE" || die "could not mount the disk image"
    APP=$(find "$MNT" -maxdepth 1 -name '*.app' | head -n 1)
    [ -n "$APP" ] || { hdiutil detach -quiet "$MNT" || true; die "no .app inside the disk image"; }
    DEST=/Applications
    if [ ! -w "$DEST" ]; then DEST="$HOME/Applications"; mkdir -p "$DEST"; fi
    NAME=$(basename "$APP")
    # copy next to the target first; the old app is replaced only after the copy succeeded
    rm -rf "${DEST:?}/.$NAME.new" "${DEST:?}/.$NAME.old"
    if ! cp -R "$APP" "$DEST/.$NAME.new"; then
      rm -rf "${DEST:?}/.$NAME.new"; hdiutil detach -quiet "$MNT" || true
      die "copy failed; the installed app (if any) was left untouched"
    fi
    hdiutil detach -quiet "$MNT" || true
    [ -e "$DEST/$NAME" ] && mv "$DEST/$NAME" "$DEST/.$NAME.old"
    if ! mv "$DEST/.$NAME.new" "$DEST/$NAME"; then
      [ -e "$DEST/.$NAME.old" ] && mv "$DEST/.$NAME.old" "$DEST/$NAME"
      die "could not put the new app in place; the old one was restored"
    fi
    rm -rf "${DEST:?}/.$NAME.old"
    # the build is not signed or notarized yet, so macOS would quarantine it
    xattr -dr com.apple.quarantine "$DEST/$(basename "$APP")" || true
    RUN="open \"$DEST/$(basename "$APP")\""
    ;;
  appimage)
    BIN="$HOME/.local/bin"; APPS="$HOME/.local/share/applications"
    mkdir -p "$BIN" "$APPS"
    LOWER=$(printf '%s' "$APP_NAME" | tr '[:upper:]' '[:lower:]')
    install -m 755 "$TMP/$FILE" "$BIN/$LOWER.AppImage"
    cat > "$APPS/$LOWER.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=$APP_NAME
Comment=Hand work to coding agents and stay away until they call you
Exec="$BIN/$LOWER.AppImage"
Terminal=false
Categories=Development;
EOF
    RUN="$BIN/$LOWER.AppImage   (or find $APP_NAME in the application menu)"
    case ":$PATH:" in *":$BIN:"*) : ;; *) say "note: $BIN is not in your PATH" ;; esac
    ;;
  deb|rpm)
    # a system package needs root; the script does not use sudo on its own
    mkdir -p "$HOME/Downloads"
    cp "$TMP/$FILE" "$HOME/Downloads/$FILE"
    if [ "$KIND" = deb ]; then CMD="sudo dpkg -i \"$HOME/Downloads/$FILE\" || sudo apt-get install -f"
    elif have zypper; then CMD="sudo zypper install \"$HOME/Downloads/$FILE\""
    else CMD="sudo dnf install \"$HOME/Downloads/$FILE\""; fi
    say "Verified package saved to $HOME/Downloads/$FILE. Install it with:"
    say "  $CMD"
    RUN="$APP_NAME in the application menu (after the command above)"
    ;;
esac

# --- 4. done ----------------------------------------------------------------------------------
step 4 "Done"
say "Run: $RUN"
say "You also need your agent CLIs (Claude Code, Codex, ...) and git; $APP_NAME runs them, it does not ship them."
