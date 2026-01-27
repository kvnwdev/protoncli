# ProtonCLI

A production-ready command-line email client for ProtonMail Bridge with Gmail-style query language support.

## Features

- **Account Management**: Add, list, remove, and test multiple ProtonMail accounts
- **Advanced Message Filtering**: Gmail-style query language for powerful message searches
- **Read Messages**: View full email content with multiple output formats (Markdown, JSON, raw)
- **Send Emails**: Compose and send emails with attachments and alias support
- **Folder Support**: List and access different mail folders
- **Agent Tracking**: Track which messages have been read by the CLI agent
- **Secure**: Passwords stored in system keychain (macOS Keychain, Linux Secret Service, Windows Credential Manager)

## Prerequisites

**ProtonMail Bridge** must be installed and running on your system:
- Download from [ProtonMail Bridge](https://proton.me/mail/bridge)
- Start Bridge and sign in to your ProtonMail account
- Note your IMAP/SMTP credentials from Bridge settings

## Installation

### From Source

```bash
git clone https://github.com/kvnwdev/protoncli.git
cd protoncli
cargo build --release
sudo cp target/release/protoncli /usr/local/bin/
```

### Homebrew (macOS/Linux)

```bash
brew tap kvnwdev/protoncli
brew install protoncli
```

## Quick Start

### 1. Add Your Account

```bash
protoncli account add user@protonmail.com
```

You'll be prompted for:
- Password (stored securely in system keychain)
- IMAP host (usually `127.0.0.1`)
- IMAP port (usually `1143`)
- SMTP host (usually `127.0.0.1`)
- SMTP port (usually `1025`)

### 2. Test Connection

```bash
protoncli account test user@protonmail.com
```

### 3. Read Your Inbox

```bash
# List recent messages
protoncli inbox

# List with previews
protoncli inbox --preview

# List unread messages
protoncli inbox --unread-only

# Search with query
protoncli inbox --query "from:github.com AND unread:true"
```

## Usage

### Account Management

```bash
# Add an account
protoncli account add user@protonmail.com

# List all accounts
protoncli account list

# Set default account
protoncli account set-default user@protonmail.com

# Remove an account
protoncli account remove user@protonmail.com

# Test account connection
protoncli account test user@protonmail.com
```

### List Folders

```bash
protoncli folders
```

### Read Messages

```bash
# Read a message (Markdown format)
protoncli read 12345

# Read with JSON output
protoncli read 12345 --output json

# Read from different folder
protoncli read 789 --folder Sent

# Show raw RFC822 message
protoncli read 12345 --raw

# Mark as read in IMAP
protoncli read 12345 --mark-read
```

### Send Emails

```bash
# Simple email
protoncli send --to recipient@example.com --subject "Hello" --body "Message text"

# Multiple recipients with CC
protoncli send --to user1@example.com --to user2@example.com --cc boss@example.com --subject "Update"

# Send from alias (uses default account's SMTP)
protoncli send --from alias@custom-domain.com --to recipient@example.com --subject "Test"

# Send with body from file
protoncli send --to recipient@example.com --subject "Report" --body-file report.txt

# Send with attachments
protoncli send --to recipient@example.com --subject "Files" --attach document.pdf --attach image.jpg

# Multiple attachments
protoncli send --to recipient@example.com --subject "Documents" --attach file1.pdf --attach file2.docx --attach photo.jpg
```

### List Inbox

```bash
# List recent messages
protoncli inbox

# Show only unread
protoncli inbox --unread-only

# Show messages not yet read by agent
protoncli inbox --agent-unread

# Limit results
protoncli inbox --limit 10

# Filter by date
protoncli inbox --days 7

# Show message previews (slower)
protoncli inbox --preview

# Output as JSON
protoncli inbox --output json

# Use query language
protoncli inbox --query "from:github.com AND subject:security"
```

## Query Language

ProtonCLI supports a Gmail-style query language for powerful message filtering.

### Basic Syntax

```bash
protoncli inbox --query "field:value"
```

### Supported Fields

**Sender & Recipients:**
```bash
from:user@example.com          # Messages from sender
to:user@example.com            # Messages to recipient
```

**Content:**
```bash
subject:invoice                # Search in subject
body:password                  # Search in body
```

**Status:**
```bash
unread:true                    # Only unread messages
is:unread                      # Alternative syntax
```

**Date Filters:**
```bash
date:>2024-01-01              # After date
date:<2024-12-31              # Before date
since:2024-01-01              # Since date (inclusive)
before:2024-02-01             # Before date (exclusive)
```

**Size:**
```bash
size:>1000000                 # Larger than 1MB
size:<5000                    # Smaller than 5KB
```

### Boolean Operators

**AND** (both conditions must match):
```bash
protoncli inbox --query "from:github.com AND unread:true"
protoncli inbox --query "from:alice@example.com subject:report"  # Implicit AND
```

**OR** (either condition must match):
```bash
protoncli inbox --query "from:alice@example.com OR from:bob@example.com"
```

**NOT** (negation):
```bash
protoncli inbox --query "subject:important NOT from:spam@example.com"
protoncli inbox --query "NOT is:unread"
```

### Complex Examples

```bash
# Unread messages from GitHub
protoncli inbox --query "from:github.com AND unread:true"

# Messages from last month
protoncli inbox --query "date:>2024-01-01 AND date:<2024-02-01"

# Important messages, excluding newsletters
protoncli inbox --query "subject:urgent NOT from:newsletter@example.com"

# Large attachments from specific domain
protoncli inbox --query "from:example.com AND size:>1000000"
```

### View Full Query Documentation

```bash
protoncli query-help
```

## Configuration

Configuration is stored at:
- **macOS/Linux**: `~/.config/protoncli/config.toml`
- **Windows**: `%APPDATA%\protoncli\config.toml`

Passwords are stored securely in:
- **macOS**: Keychain
- **Linux**: Secret Service (libsecret)
- **Windows**: Credential Manager

### Example Configuration

```toml
[[accounts]]
email = "user@protonmail.com"
imap_host = "127.0.0.1"
imap_port = 1143
imap_security = "StartTls"
smtp_host = "127.0.0.1"
smtp_port = 1025
smtp_security = "StartTls"
default = true

[preferences]
default_output = "json"
date_filter_days = 3
cache_enabled = true
log_level = "info"
```

## Development

### Prerequisites

- Rust 1.70 or later
- ProtonMail Bridge running locally

### Build

```bash
cargo build
```

### Run Tests

```bash
# Run all tests
cargo test

# Run with visible output
cargo test -- --nocapture

# Run specific module tests
cargo test models::query
cargo test models::filter
cargo test cli::query
cargo test cli::actions

# Run with verbose output
cargo test --verbose
```

### Run

```bash
cargo run -- inbox
cargo run -- send --to user@example.com --subject "Test"
```

## Releasing a New Version

1. **Build and test locally:**
   ```bash
   cargo build --release
   cargo test
   ```

2. **Create and push a new tag:**
   ```bash
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

3. **Create GitHub Release:**
   - Go to https://github.com/kvnwdev/protoncli/releases
   - Click "Create a new release"
   - Select the tag, add release notes, publish

4. **Update Homebrew formula:**
   ```bash
   # Get new SHA256
   curl -sL https://github.com/kvnwdev/protoncli/archive/refs/tags/vX.Y.Z.tar.gz | shasum -a 256

   # Update homebrew-protoncli/Formula/protoncli.rb with new version and sha256
   cd /path/to/homebrew-protoncli
   # Edit Formula/protoncli.rb
   git commit -am "Update to vX.Y.Z"
   git push
   ```

## Architecture

```
protoncli/
├── src/
│   ├── cli/           # Command handlers
│   │   ├── account.rs
│   │   ├── folder.rs
│   │   ├── message.rs
│   │   └── send.rs
│   ├── core/          # Core functionality
│   │   ├── auth.rs    # Keychain integration
│   │   ├── imap.rs    # IMAP client
│   │   ├── smtp.rs    # SMTP client
│   │   └── state.rs   # SQLite state tracking
│   ├── models/        # Data models
│   │   ├── account.rs
│   │   ├── config.rs
│   │   ├── filter.rs
│   │   ├── folder.rs
│   │   ├── message.rs
│   │   └── query.rs
│   ├── output/        # Output formatters
│   │   ├── json.rs
│   │   └── markdown.rs
│   └── main.rs        # CLI entry point
├── migrations/        # SQLite migrations
└── Cargo.toml
```

## Troubleshooting

### "Password not found in keychain"

Make sure you've added the account:
```bash
protoncli account add user@protonmail.com
```

### "Failed to connect to IMAP server"

1. Ensure ProtonMail Bridge is running
2. Verify IMAP settings in Bridge
3. Test connection: `protoncli account test user@protonmail.com`

### "No default account configured"

Set a default account:
```bash
protoncli account set-default user@protonmail.com
```

### SMTP Errors When Using Aliases

If you get SMTP errors when sending from an alias:
1. Verify the alias is configured in your ProtonMail account
2. Check that ProtonMail Bridge allows sending from that alias
3. Try sending from the main account address first to verify SMTP works

## License

MIT

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Support

For issues and questions, please open an issue on GitHub.
