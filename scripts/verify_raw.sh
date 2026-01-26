#!/bin/bash
# Raw verification script for ProtonMail Bridge connection

set -e

EMAIL="${1:-kevinwilloughby@protonmail.com}"
IMAP_HOST="127.0.0.1"
IMAP_PORT="1143"
SMTP_HOST="127.0.0.1"
SMTP_PORT="1025"

echo "================================================"
echo "ProtonMail Bridge Raw Connection Test"
echo "================================================"
echo

# Test 1: Check if ports are listening
echo "1. Checking if Bridge ports are listening..."
echo "   IMAP port 1143:"
if nc -z -w 1 $IMAP_HOST $IMAP_PORT 2>/dev/null; then
    echo "   ✓ Port 1143 is open"
else
    echo "   ✗ Port 1143 is NOT open - is Bridge running?"
    exit 1
fi

echo "   SMTP port 1025:"
if nc -z -w 1 $SMTP_HOST $SMTP_PORT 2>/dev/null; then
    echo "   ✓ Port 1025 is open"
else
    echo "   ✗ Port 1025 is NOT open - is Bridge running?"
    exit 1
fi
echo

# Test 2: Check if it's immediate TLS (like IMAPS)
echo "2. Testing immediate TLS connection..."
(
    echo "QUIT" | openssl s_client -connect $IMAP_HOST:$IMAP_PORT -quiet 2>&1 | head -5 &
    PID=$!
    sleep 2
    kill $PID 2>/dev/null || true
)
echo

# Test 3: Try plain text connection to see server greeting
echo "3. Testing plain text IMAP connection..."
echo "   Connecting to get server greeting..."
(
    (echo "A001 CAPABILITY"; sleep 1; echo "A002 LOGOUT") | nc $IMAP_HOST $IMAP_PORT 2>&1 &
    PID=$!
    sleep 2
    kill $PID 2>/dev/null || true
) | head -10
echo

# Test 4: Get password from keychain
echo "4. Checking keychain for password..."
if security find-generic-password -s protoncli -a "$EMAIL" >/dev/null 2>&1; then
    echo "   ✓ Password found in keychain for $EMAIL"
else
    echo "   ✗ Password NOT found in keychain for $EMAIL"
    echo "   Run: security add-generic-password -s protoncli -a '$EMAIL' -w"
    exit 1
fi
echo

# Test 5: Check Bridge settings
echo "5. Bridge Configuration Check:"
echo "   Expected IMAP: $IMAP_HOST:$IMAP_PORT"
echo "   Expected SMTP: $SMTP_HOST:$SMTP_PORT"
echo
echo "   Please verify in ProtonMail Bridge app that:"
echo "   - Bridge is running and connected"
echo "   - IMAP port is 1143"
echo "   - SMTP port is 1025"
echo "   - Your account is logged in"
echo

# Test 6: Try with openssl and STARTTLS
echo "6. Testing STARTTLS capability..."
(
    (echo "A001 CAPABILITY"; sleep 1) | openssl s_client -connect $IMAP_HOST:$IMAP_PORT -starttls imap -quiet 2>&1 &
    PID=$!
    sleep 2
    kill $PID 2>/dev/null || true
) | grep -E "(CAPABILITY|OK|BAD|NO|Connected)" || echo "   (No clear IMAP response - may need direct TLS)"
echo

echo "================================================"
echo "Summary:"
echo "================================================"
echo "If you see 'OK' responses above, Bridge is working."
echo "If you see connection refused, Bridge is not running."
echo "If you see TLS errors, the connection mode might be wrong."
echo
