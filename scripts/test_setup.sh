#!/bin/bash
# Test setup helper for ProtonCLI

set -e

EMAIL="${1:-kevinwilloughby@protonmail.com}"

echo "Setting up ProtonCLI for testing..."
echo

# Create config directory (macOS uses ~/Library/Application Support)
mkdir -p "$HOME/Library/Application Support/protoncli"

# Create config file
cat > "$HOME/Library/Application Support/protoncli/config.toml" <<EOF
[[accounts]]
email = "$EMAIL"
imap_host = "127.0.0.1"
imap_port = 1143
imap_security = "starttls"
smtp_host = "127.0.0.1"
smtp_port = 1025
smtp_security = "ssl"
default = true

[preferences]
default_output = "json"
date_filter_days = 3
cache_enabled = true
log_level = "info"
EOF

echo "✓ Created config at ~/Library/Application Support/protoncli/config.toml"
echo
echo "Now you need to store your ProtonMail Bridge password in the keychain:"
echo
echo "Run this command:"
echo "  security add-generic-password -s protoncli -a '$EMAIL' -w"
echo
echo "It will prompt you for the password (use your Bridge password, not ProtonMail password)"
echo
echo "After that, test the connection:"
echo "  ./target/debug/protoncli account test $EMAIL"
echo
