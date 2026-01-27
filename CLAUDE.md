# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
cargo build              # Development build
cargo build --release    # Release build with optimizations
cargo test               # Run all tests
cargo run -- <command>   # Run CLI with arguments (e.g., cargo run -- inbox)
```

## Testing

### Run Tests

```bash
cargo test                    # Run all tests
cargo test -- --nocapture     # Run with stdout visible
cargo test models::query      # Run specific module tests
cargo test cli::query         # Run CLI query tests
```

### Test Structure

- **Unit tests**: Located in same files as code (`#[cfg(test)]` modules)
- **Test coverage**: Query parsing, filter translation, folder resolution
- **CI**: GitHub Actions runs tests on every push/PR

## Architecture

ProtonCLI is a CLI email client for ProtonMail Bridge, built with async Rust using Tokio.

### Module Structure

- **main.rs**: CLI entry point using Clap derive macros. Contains `Commands` enum defining all subcommands (Account, Folders, Inbox, Read, Send, Move, Copy, Delete, Archive, Flag).

- **cli/**: Command handlers that orchestrate core functionality
  - `account.rs` - Account CRUD operations
  - `message.rs` - Inbox listing and message reading
  - `send.rs` - Email composition and sending
  - `actions.rs` - Message operations (move, copy, delete, archive, flag)
  - `folder.rs` - Folder listing

- **core/**: Protocol implementations
  - `imap.rs` - async-imap client wrapper for IMAP operations
  - `smtp.rs` - lettre-based SMTP client
  - `auth.rs` - System keychain integration via keyring crate
  - `state.rs` - SQLite-based state tracking (agent read status)

- **models/**: Data structures
  - `config.rs` - TOML configuration (accounts, preferences)
  - `query.rs` - Gmail-style query language parser
  - `filter.rs` - Message filtering logic
  - `message.rs`, `account.rs`, `folder.rs` - Domain models

- **output/**: Formatters for JSON and Markdown output

### Key Dependencies

- **async-imap** + **async-native-tls**: IMAP with TLS
- **lettre**: SMTP client
- **mail-parser**: RFC822 parsing
- **keyring**: Cross-platform secure credential storage
- **sqlx** (SQLite): State persistence for agent tracking
- **clap** (derive): CLI argument parsing

### Data Storage

- Config: `~/.config/protoncli/config.toml`
- State DB: `~/.config/protoncli/protoncli.db` (SQLite)
- Passwords: System keychain (macOS Keychain, Linux Secret Service, Windows Credential Manager)

### Query Language

The `models/query.rs` module implements a Gmail-style query parser supporting fields like `from:`, `to:`, `subject:`, `body:`, `unread:`, `date:`, `size:` with AND/OR/NOT operators.
