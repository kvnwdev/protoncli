# ProtonCLI

ProtonMail in your terminal. Works with ProtonMail Bridge to give you IMAP/SMTP access from the command line. Search, read, send, and script your email.

- Query syntax you already know (Gmail-style)
- JSON output for piping to `jq`, scripts, or wherever
- Passwords stored in system keychain

## Requirements

[ProtonMail Bridge](https://proton.me/mail/bridge) must be running. Install it, sign in, and grab your IMAP/SMTP credentials from Bridge settings.

## Install

**Homebrew:**
```bash
brew tap kvnwdev/protoncli
brew install protoncli
```

**From source:**
```bash
git clone https://github.com/kvnwdev/protoncli.git
cd protoncli
cargo build --release
cp target/release/protoncli /usr/local/bin/
```

## Setup

Add your account:
```bash
protoncli account add user@protonmail.com
```

You'll enter your Bridge password (stored in system keychain) and connection details (usually `127.0.0.1:1143` for IMAP, `127.0.0.1:1025` for SMTP).

Test it:
```bash
protoncli account test user@protonmail.com
```

## Usage

### Read your inbox

```bash
protoncli inbox                          # list recent messages
protoncli inbox --unread-only            # just unread
protoncli inbox --preview                # include message previews
protoncli inbox --output json            # for scripting
protoncli inbox --query "from:github.com AND unread:true"
```

### Read a message

```bash
protoncli read 12345                     # markdown output
protoncli read 12345 --output json       # json output
protoncli read 12345 --raw               # raw RFC822
protoncli read 12345 --mark-read         # mark as read in IMAP
```

### Send email

```bash
protoncli send --to user@example.com --subject "Hello" --body "Message"
protoncli send --to user@example.com --subject "Report" --body-file report.txt
protoncli send --to user@example.com --attach doc.pdf --attach image.jpg
```

### Other commands

```bash
protoncli folders                        # list folders
protoncli account list                   # list accounts
protoncli account set-default user@...   # set default account
```

## Query language

Gmail-style queries for filtering messages:

```bash
# fields
from:user@example.com
to:user@example.com
subject:invoice
body:password
unread:true

# dates
date:>2024-01-01
date:<2024-12-31
since:2024-01-01
before:2024-02-01

# size (bytes)
size:>1000000
size:<5000

# combine with AND, OR, NOT
protoncli inbox --query "from:github.com AND unread:true"
protoncli inbox --query "from:alice OR from:bob"
protoncli inbox --query "subject:urgent NOT from:newsletter@example.com"
```

Run `protoncli query-help` for the full reference.

## Scripting examples

Unread count:
```bash
protoncli inbox --unread-only --output json | jq length
```

Subjects from a sender:
```bash
protoncli inbox --query "from:github.com" --output json | jq -r '.[].subject'
```

Process new messages in a loop:
```bash
protoncli inbox --agent-unread --output json | jq -r '.[].uid' | while read uid; do
  protoncli read "$uid" --mark-read
done
```

## Configuration

Config lives at `~/.config/protoncli/config.toml`:

```toml
[[accounts]]
email = "user@protonmail.com"
imap_host = "127.0.0.1"
imap_port = 1143
smtp_host = "127.0.0.1"
smtp_port = 1025
default = true

[preferences]
default_output = "json"
date_filter_days = 3
```

## Troubleshooting

**"Password not found in keychain"** — Run `protoncli account add` first.

**"Failed to connect to IMAP server"** — Make sure ProtonMail Bridge is running. Check your port settings with `protoncli account test`.

**"No default account configured"** — Run `protoncli account set-default user@protonmail.com`.

## Development

Requires Rust 1.70+ and ProtonMail Bridge running locally.

```bash
cargo build
cargo test
cargo run -- inbox
```

Enable the pre-commit hook for formatting:
```bash
git config core.hooksPath .githooks
```

## License

MIT
