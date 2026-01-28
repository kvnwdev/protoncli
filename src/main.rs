mod cli;
mod core;
mod models;
mod output;
mod utils;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "protoncli")]
#[command(version = "0.4.0")]
#[command(about = "A production-ready CLI email client for ProtonMail Bridge", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Account management commands
    Account {
        #[command(subcommand)]
        action: AccountAction,
    },
    /// Folder management commands
    Folders {
        #[command(subcommand)]
        action: Option<FolderAction>,
    },
    /// Label management commands
    Labels {
        #[command(subcommand)]
        action: Option<LabelAction>,
    },
    /// Search messages with Gmail-style query
    Query {
        /// Query expression (Gmail-style syntax)
        query: String,
        /// Folder(s) to search (can be specified multiple times)
        #[arg(long, short = 'F')]
        folder: Vec<String>,
        /// Comma-separated fields to display (uid,subject,from,date,flags,message_id)
        #[arg(long, short = 'f')]
        fields: Option<String>,
        /// Output format (json, markdown, text)
        #[arg(long, short)]
        output: Option<String>,
        /// Add results to selection
        #[arg(long)]
        select: bool,
        /// Limit number of results
        #[arg(long)]
        limit: Option<usize>,
        /// Include body preview
        #[arg(long)]
        preview: bool,
    },
    /// Manage message selection
    Select {
        #[command(subcommand)]
        action: SelectAction,
    },
    /// View and manage pending draft operations
    Draft {
        #[command(subcommand)]
        action: DraftAction,
    },
    /// List inbox messages
    Inbox {
        /// Filter messages from the last N days
        #[arg(long)]
        days: Option<u32>,
        /// Show only unread messages
        #[arg(long)]
        unread_only: bool,
        /// Show only messages not yet read by the agent
        #[arg(long)]
        agent_unread: bool,
        /// Limit number of messages
        #[arg(long)]
        limit: Option<usize>,
        /// Output format (json, markdown, or table)
        #[arg(long, short)]
        output: Option<String>,
        /// Query expression (Gmail-style syntax)
        #[arg(long, short = 'q', value_name = "QUERY")]
        query: Option<String>,
        /// Fetch message body previews (slower but shows content)
        #[arg(long)]
        preview: bool,
    },
    /// Read a message
    Read {
        /// Message ID (shadow UID from inbox/query output)
        id: i64,
        /// Folder to read from (optional, resolved from shadow UID)
        #[arg(long)]
        folder: Option<String>,
        /// Output format (json, markdown, raw)
        #[arg(long, short, default_value = "markdown")]
        output: String,
        /// Mark message as read in IMAP
        #[arg(long)]
        mark_read: bool,
        /// Show raw RFC822 message
        #[arg(long)]
        raw: bool,
    },
    /// Send an email
    Send {
        /// Sender email address (defaults to default account if not specified)
        #[arg(long, short)]
        from: Option<String>,
        /// Recipient email address(es)
        #[arg(long, required = true)]
        to: Vec<String>,
        /// CC recipient(s)
        #[arg(long)]
        cc: Vec<String>,
        /// BCC recipient(s)
        #[arg(long)]
        bcc: Vec<String>,
        /// Email subject
        #[arg(long, short = 's')]
        subject: Option<String>,
        /// Email body text
        #[arg(long, short)]
        body: Option<String>,
        /// Read body from file
        #[arg(long)]
        body_file: Option<String>,
        /// Attach files
        #[arg(long)]
        attach: Vec<String>,
    },
    /// Show query language documentation
    QueryHelp,
    /// Move messages to another folder
    Move {
        /// Message ID(s) (shadow UIDs from inbox/query output)
        ids: Vec<i64>,
        /// Destination folder
        #[arg(long, required = true)]
        to: String,
        /// Use selection instead of IDs
        #[arg(long, conflicts_with = "ids")]
        selection: bool,
        /// Stage action as draft without executing
        #[arg(long)]
        draft: bool,
        /// Preserve selection after action (default: clear)
        #[arg(long)]
        keep: bool,
        /// Output format (json or text)
        #[arg(long, short)]
        output: Option<String>,
    },
    /// Copy messages to another folder
    Copy {
        /// Message ID(s) (shadow UIDs from inbox/query output)
        ids: Vec<i64>,
        /// Destination folder
        #[arg(long, required = true)]
        to: String,
        /// Use selection instead of IDs
        #[arg(long, conflicts_with = "ids")]
        selection: bool,
        /// Stage action as draft without executing
        #[arg(long)]
        draft: bool,
        /// Preserve selection after action (default: clear)
        #[arg(long)]
        keep: bool,
        /// Output format (json or text)
        #[arg(long, short)]
        output: Option<String>,
    },
    /// Delete messages (move to Trash or permanent delete)
    Delete {
        /// Message ID(s) (shadow UIDs from inbox/query output)
        ids: Vec<i64>,
        /// Permanently delete (bypasses Trash)
        #[arg(long)]
        permanent: bool,
        /// Skip confirmation for permanent delete
        #[arg(long, short)]
        yes: bool,
        /// Use selection instead of IDs
        #[arg(long, conflicts_with = "ids")]
        selection: bool,
        /// Stage action as draft without executing
        #[arg(long)]
        draft: bool,
        /// Preserve selection after action (default: clear)
        #[arg(long)]
        keep: bool,
        /// Output format (json or text)
        #[arg(long, short)]
        output: Option<String>,
    },
    /// Archive messages (move to Archive folder)
    Archive {
        /// Message ID(s) (shadow UIDs from inbox/query output)
        ids: Vec<i64>,
        /// Use selection instead of IDs
        #[arg(long, conflicts_with = "ids")]
        selection: bool,
        /// Stage action as draft without executing
        #[arg(long)]
        draft: bool,
        /// Preserve selection after action (default: clear)
        #[arg(long)]
        keep: bool,
        /// Output format (json or text)
        #[arg(long, short)]
        output: Option<String>,
    },
    /// Modify message flags (read/unread, starred, labels) and optionally move
    Flag {
        /// Message ID(s) (shadow UIDs from inbox/query output)
        ids: Vec<i64>,
        /// Mark as read
        #[arg(long)]
        read: bool,
        /// Mark as unread
        #[arg(long)]
        unread: bool,
        /// Star the message
        #[arg(long)]
        starred: bool,
        /// Unstar the message
        #[arg(long)]
        unstarred: bool,
        /// Add label(s)
        #[arg(long = "label")]
        labels: Vec<String>,
        /// Remove label(s)
        #[arg(long = "unlabel")]
        unlabels: Vec<String>,
        /// Move to folder after applying flags
        #[arg(long = "move")]
        move_to: Option<String>,
        /// Use selection instead of IDs
        #[arg(long, conflicts_with = "ids")]
        selection: bool,
        /// Stage action as draft without executing
        #[arg(long)]
        draft: bool,
        /// Preserve selection after action (default: clear)
        #[arg(long)]
        keep: bool,
        /// Output format (json or text)
        #[arg(long, short)]
        output: Option<String>,
    },
}

#[derive(Subcommand)]
enum AccountAction {
    /// Add a new account
    Add {
        /// Email address
        email: String,
    },
    /// List all accounts
    List,
    /// Set default account
    SetDefault {
        /// Email address
        email: String,
    },
    /// Remove an account
    Remove {
        /// Email address
        email: String,
    },
    /// Test account connection
    Test {
        /// Email address
        email: String,
    },
}

#[derive(Subcommand)]
enum SelectAction {
    /// Add IDs to selection
    Add {
        /// Message IDs (shadow UIDs from inbox/query output)
        ids: Vec<i64>,
        /// Output format (json or text)
        #[arg(long, short)]
        output: Option<String>,
    },
    /// Add last query results to selection
    Last {
        /// Folder to get last query results from
        #[arg(long, default_value = "INBOX")]
        folder: String,
        /// Output format (json or text)
        #[arg(long, short)]
        output: Option<String>,
    },
    /// Remove IDs from selection
    Remove {
        /// Message IDs (shadow UIDs from inbox/query output)
        ids: Vec<i64>,
        /// Output format (json or text)
        #[arg(long, short)]
        output: Option<String>,
    },
    /// Show current selection
    Show {
        /// Output format (json or text)
        #[arg(long, short)]
        output: Option<String>,
    },
    /// Clear selection
    Clear {
        /// Output format (json or text)
        #[arg(long, short)]
        output: Option<String>,
    },
    /// Count messages in selection
    Count {
        /// Output format (json or text)
        #[arg(long, short)]
        output: Option<String>,
    },
}

#[derive(Subcommand)]
enum DraftAction {
    /// Show the current draft
    Show {
        /// Output format (json or text)
        #[arg(long, short)]
        output: Option<String>,
    },
    /// Clear the current draft
    Clear {
        /// Output format (json or text)
        #[arg(long, short)]
        output: Option<String>,
    },
}

#[derive(Subcommand)]
enum FolderAction {
    /// List all folders
    List {
        /// Output format (json)
        #[arg(long, short)]
        output: Option<String>,
    },
    /// Create a new folder
    Create {
        /// Folder name (can include path like "Projects/Work")
        name: String,
        /// Output format (json)
        #[arg(long, short)]
        output: Option<String>,
    },
    /// Delete a folder (must be empty)
    Delete {
        /// Folder name
        name: String,
        /// Output format (json)
        #[arg(long, short)]
        output: Option<String>,
    },
    /// Rename a folder
    Rename {
        /// Current folder name
        from: String,
        /// New folder name
        to: String,
        /// Output format (json)
        #[arg(long, short)]
        output: Option<String>,
    },
}

#[derive(Subcommand)]
enum LabelAction {
    /// List all labels
    List {
        /// Output format (json)
        #[arg(long, short)]
        output: Option<String>,
    },
    /// Create a new label
    Create {
        /// Label name
        name: String,
        /// Output format (json)
        #[arg(long, short)]
        output: Option<String>,
    },
    /// Delete a label (must be empty)
    Delete {
        /// Label name
        name: String,
        /// Output format (json)
        #[arg(long, short)]
        output: Option<String>,
    },
    /// Rename a label
    Rename {
        /// Current label name
        from: String,
        /// New label name
        to: String,
        /// Output format (json)
        #[arg(long, short)]
        output: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Account { action } => match action {
            AccountAction::Add { email } => cli::account::add_account(&email).await?,
            AccountAction::List => cli::account::list_accounts()?,
            AccountAction::SetDefault { email } => cli::account::set_default_account(&email)?,
            AccountAction::Remove { email } => cli::account::remove_account(&email)?,
            AccountAction::Test { email } => cli::account::test_account(&email).await?,
        },
        Commands::Folders { action } => match action {
            None | Some(FolderAction::List { output: None }) => {
                cli::folder::list_folders(None).await?
            }
            Some(FolderAction::List { output }) => {
                cli::folder::list_folders(output.as_deref()).await?
            }
            Some(FolderAction::Create { name, output }) => {
                cli::folder::create_folder(&name, output.as_deref()).await?
            }
            Some(FolderAction::Delete { name, output }) => {
                cli::folder::delete_folder(&name, output.as_deref()).await?
            }
            Some(FolderAction::Rename { from, to, output }) => {
                cli::folder::rename_folder(&from, &to, output.as_deref()).await?
            }
        },
        Commands::Labels { action } => match action {
            None | Some(LabelAction::List { output: None }) => {
                cli::label::list_labels(None).await?
            }
            Some(LabelAction::List { output }) => {
                cli::label::list_labels(output.as_deref()).await?
            }
            Some(LabelAction::Create { name, output }) => {
                cli::label::create_label(&name, output.as_deref()).await?
            }
            Some(LabelAction::Delete { name, output }) => {
                cli::label::delete_label(&name, output.as_deref()).await?
            }
            Some(LabelAction::Rename { from, to, output }) => {
                cli::label::rename_label(&from, &to, output.as_deref()).await?
            }
        },
        Commands::Query {
            query,
            folder,
            fields,
            output,
            select,
            limit,
            preview,
        } => {
            cli::query::execute_query(
                &query,
                &folder,
                fields.as_deref(),
                limit,
                preview,
                select,
                output.as_deref(),
            )
            .await?
        }
        Commands::Select { action } => match action {
            SelectAction::Add { ids, output } => {
                cli::select::add_to_selection(ids, output.as_deref()).await?
            }
            SelectAction::Last { folder, output } => {
                cli::select::add_last_query_to_selection(&folder, output.as_deref()).await?
            }
            SelectAction::Remove { ids, output } => {
                cli::select::remove_from_selection(ids, output.as_deref()).await?
            }
            SelectAction::Show { output } => cli::select::show_selection(output.as_deref()).await?,
            SelectAction::Clear { output } => {
                cli::select::clear_selection(output.as_deref()).await?
            }
            SelectAction::Count { output } => {
                cli::select::count_selection(output.as_deref()).await?
            }
        },
        Commands::Draft { action } => match action {
            DraftAction::Show { output } => cli::draft::show_draft(output.as_deref()).await?,
            DraftAction::Clear { output } => cli::draft::clear_draft(output.as_deref()).await?,
        },
        Commands::Inbox {
            days,
            unread_only,
            agent_unread,
            limit,
            output,
            query,
            preview,
        } => {
            cli::message::list_inbox(
                days,
                unread_only,
                agent_unread,
                limit,
                output.as_deref(),
                query,
                preview,
            )
            .await?
        }
        Commands::Read {
            id,
            folder,
            output,
            mark_read,
            raw,
        } => {
            cli::message::read_message(id, folder.as_deref(), Some(&output), mark_read, raw).await?
        }
        Commands::Send {
            from,
            to,
            cc,
            bcc,
            subject,
            body,
            body_file,
            attach,
        } => cli::send::send_email(from, to, cc, bcc, subject, body, body_file, attach).await?,
        Commands::QueryHelp => show_query_help(),
        Commands::Move {
            ids,
            to,
            selection,
            draft,
            keep,
            output,
        } => {
            cli::actions::move_messages(ids, &to, selection, draft, keep, output.as_deref()).await?
        }
        Commands::Copy {
            ids,
            to,
            selection,
            draft,
            keep,
            output,
        } => {
            cli::actions::copy_messages(ids, &to, selection, draft, keep, output.as_deref()).await?
        }
        Commands::Delete {
            ids,
            permanent,
            yes,
            selection,
            draft,
            keep,
            output,
        } => {
            cli::actions::delete_messages(
                ids,
                permanent,
                yes,
                selection,
                draft,
                keep,
                output.as_deref(),
            )
            .await?
        }
        Commands::Archive {
            ids,
            selection,
            draft,
            keep,
            output,
        } => cli::actions::archive_messages(ids, selection, draft, keep, output.as_deref()).await?,
        Commands::Flag {
            ids,
            read,
            unread,
            starred,
            unstarred,
            labels,
            unlabels,
            move_to,
            selection,
            draft,
            keep,
            output,
        } => {
            cli::actions::modify_flags(
                ids,
                read,
                unread,
                starred,
                unstarred,
                labels,
                unlabels,
                move_to,
                selection,
                draft,
                keep,
                output.as_deref(),
            )
            .await?
        }
    }

    Ok(())
}

fn show_query_help() {
    // Load help text from asset file at compile time
    const HELP_TEXT: &str = include_str!("assets/query_help.txt");

    // ANSI color codes
    const BOLD: &str = "\x1b[1m";
    const CYAN: &str = "\x1b[36m";
    const YELLOW: &str = "\x1b[33m";
    const RESET: &str = "\x1b[0m";

    // Process each line and apply formatting
    for line in HELP_TEXT.lines() {
        let formatted = if let Some(stripped) = line.strip_prefix("# ") {
            // Main title: bold
            format!("{}{}{}", BOLD, stripped, RESET)
        } else if let Some(stripped) = line.strip_prefix("## ") {
            // Section headers: bold
            format!("\n{}{}{}", BOLD, stripped, RESET)
        } else {
            // Process inline markup
            let mut result = line.to_string();

            // Process `text` -> cyan (field names, flags)
            while let Some(start) = result.find('`') {
                if let Some(end) = result[start + 1..].find('`') {
                    let end = start + 1 + end;
                    let inner = &result[start + 1..end];
                    result = format!(
                        "{}{}{}{}{}",
                        &result[..start],
                        CYAN,
                        inner,
                        RESET,
                        &result[end + 1..]
                    );
                } else {
                    break;
                }
            }

            // Process **text** -> bold
            while let Some(start) = result.find("**") {
                if let Some(end) = result[start + 2..].find("**") {
                    let end = start + 2 + end;
                    let inner = &result[start + 2..end];
                    result = format!(
                        "{}{}{}{}{}",
                        &result[..start],
                        BOLD,
                        inner,
                        RESET,
                        &result[end + 2..]
                    );
                } else {
                    break;
                }
            }

            // Process ~text~ -> yellow (operators like AND, OR, NOT)
            while let Some(start) = result.find('~') {
                if let Some(end) = result[start + 1..].find('~') {
                    let end = start + 1 + end;
                    let inner = &result[start + 1..end];
                    result = format!(
                        "{}{}{}{}{}",
                        &result[..start],
                        YELLOW,
                        inner,
                        RESET,
                        &result[end + 1..]
                    );
                } else {
                    break;
                }
            }

            result
        };

        println!("{}", formatted);
    }
}
