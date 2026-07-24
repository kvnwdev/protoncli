#!/usr/bin/env bash
# Build, sign, and install Proton CLI for this Mac. Mirrors Peri's personal
# alpha signing flow: staged artifact, Apple Development signing, then verify.
# This is for local use only; it does not notarize or publish a release.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
install_dir="${PROTONCLI_INSTALL_DIR:-$HOME/.local/bin}"
identity="${PROTONCLI_SIGNING_IDENTITY:-auto}"
dry_run=false

usage() {
  cat <<'EOF'
Usage: scripts/install-signed-local.sh [--identity NAME] [--install-dir PATH] [--dry-run]

Builds the release binary, signs it with an Apple Development identity, and
installs it as protoncli. Set PROTONCLI_SIGNING_IDENTITY to avoid selecting an
identity interactively. The default destination is ~/.local/bin.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --identity) identity="$2"; shift 2 ;;
    --install-dir) install_dir="$2"; shift 2 ;;
    --dry-run) dry_run=true; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
done

if [[ "$identity" == "auto" ]]; then
    identity="$({ security find-identity -v -p codesigning | \
    sed -n 's/.*\"\(Apple Development:.*\)\"/\1/p' | head -n 1; } || true)"
  if [[ -z "$identity" ]]; then
    identity="-"
  fi
fi

staging_root="$(mktemp -d "${TMPDIR:-/tmp}/protoncli-package.XXXXXX")"
trap 'rm -rf "$staging_root"' EXIT
staged_binary="$staging_root/protoncli"
if "$dry_run"; then
  printf 'Would build release binary, sign with: %s\n' "$identity"
  printf 'Would install: %s\n' "$install_dir/protoncli"
  exit 0
fi

cd "$repo_root"
cargo build --release --locked
ditto "$repo_root/target/release/protoncli" "$staged_binary"
codesign --force --options runtime --timestamp=none --sign "$identity" "$staged_binary"
codesign --verify --strict --verbose=2 "$staged_binary"

mkdir -p "$install_dir"
rm -f "$install_dir/protoncli"
ditto "$staged_binary" "$install_dir/protoncli"
codesign --verify --strict --verbose=2 "$install_dir/protoncli"

echo "Installed signed protoncli at $install_dir/protoncli"
echo "Signing identity: $identity"
echo "Run this command once, then choose Always Allow if macOS asks for Keychain access:"
echo "  $install_dir/protoncli account test <your-account-email>"
