mod cli;
mod core;
mod models;
mod output;
mod utils;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "protoncli")]
#[command(version = "0.3.0")]
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
    /// List folders
    Folders,
    /// Search messages with Gmail-style query
    Query {
        /// Query expression (Gmail-style syntax)
        query: String,
        /// Folder to search
        #[arg(long, default_value = "INBOX")]
        folder: String,
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
        /// Output format (json or markdown)
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
        /// Message UID
        uid: u32,
        /// Folder to read from
        #[arg(long, default_value = "INBOX")]
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
        /// Message UID(s)
        uids: Vec<u32>,
        /// Source folder
        #[arg(long, default_value = "INBOX")]
        from: String,
        /// Destination folder
        #[arg(long, required = true)]
        to: String,
        /// Use selection instead of UIDs
        #[arg(long, conflicts_with = "uids")]
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
        /// Message UID(s)
        uids: Vec<u32>,
        /// Source folder
        #[arg(long, default_value = "INBOX")]
        from: String,
        /// Destination folder
        #[arg(long, required = true)]
        to: String,
        /// Use selection instead of UIDs
        #[arg(long, conflicts_with = "uids")]
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
        /// Message UID(s)
        uids: Vec<u32>,
        /// Source folder
        #[arg(long, default_value = "INBOX")]
        from: String,
        /// Permanently delete (bypasses Trash)
        #[arg(long)]
        permanent: bool,
        /// Skip confirmation for permanent delete
        #[arg(long, short)]
        yes: bool,
        /// Use selection instead of UIDs
        #[arg(long, conflicts_with = "uids")]
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
        /// Message UID(s)
        uids: Vec<u32>,
        /// Source folder
        #[arg(long, default_value = "INBOX")]
        from: String,
        /// Use selection instead of UIDs
        #[arg(long, conflicts_with = "uids")]
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
        /// Message UID(s)
        uids: Vec<u32>,
        /// Source folder
        #[arg(long, default_value = "INBOX")]
        from: String,
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
        /// Use selection instead of UIDs
        #[arg(long, conflicts_with = "uids")]
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
    /// Add UIDs to selection
    Add {
        /// Message UIDs
        uids: Vec<u32>,
        /// Folder containing the messages
        #[arg(long, default_value = "INBOX")]
        folder: String,
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
    /// Remove UIDs from selection
    Remove {
        /// Message UIDs
        uids: Vec<u32>,
        /// Folder containing the messages
        #[arg(long, default_value = "INBOX")]
        folder: String,
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
        Commands::Folders => cli::folder::list_folders(None).await?,
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
            SelectAction::Add { uids, folder, output } => {
                cli::select::add_to_selection(uids, &folder, output.as_deref()).await?
            }
            SelectAction::Last { folder, output } => {
                cli::select::add_last_query_to_selection(&folder, output.as_deref()).await?
            }
            SelectAction::Remove { uids, folder, output } => {
                cli::select::remove_from_selection(uids, &folder, output.as_deref()).await?
            }
            SelectAction::Show { output } => {
                cli::select::show_selection(output.as_deref()).await?
            }
            SelectAction::Clear { output } => {
                cli::select::clear_selection(output.as_deref()).await?
            }
            SelectAction::Count { output } => {
                cli::select::count_selection(output.as_deref()).await?
            }
        },
        Commands::Draft { action } => match action {
            DraftAction::Show { output } => {
                cli::draft::show_draft(output.as_deref()).await?
            }
            DraftAction::Clear { output } => {
                cli::draft::clear_draft(output.as_deref()).await?
            }
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
            cli::message::list_inbox(days, unread_only, agent_unread, limit, output.as_deref(), query, preview)
                .await?
        }
        Commands::Read {
            uid,
            folder,
            output,
            mark_read,
            raw,
        } => {
            cli::message::read_message(uid, folder.as_deref(), Some(&output), mark_read, raw)
                .await?
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
        } => {
            cli::send::send_email(from, to, cc, bcc, subject, body, body_file, attach).await?
        }
        Commands::QueryHelp => show_query_help(),
        Commands::Move {
            uids,
            from,
            to,
            selection,
            draft,
            keep,
            output,
        } => cli::actions::move_messages(uids, &from, &to, selection, draft, keep, output.as_deref()).await?,
        Commands::Copy {
            uids,
            from,
            to,
            selection,
            draft,
            keep,
            output,
        } => cli::actions::copy_messages(uids, &from, &to, selection, draft, keep, output.as_deref()).await?,
        Commands::Delete {
            uids,
            from,
            permanent,
            yes,
            selection,
            draft,
            keep,
            output,
        } => cli::actions::delete_messages(uids, &from, permanent, yes, selection, draft, keep, output.as_deref()).await?,
        Commands::Archive { uids, from, selection, draft, keep, output } => {
            cli::actions::archive_messages(uids, &from, selection, draft, keep, output.as_deref()).await?
        }
        Commands::Flag {
            uids,
            from,
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
                uids,
                &from,
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
    // ANSI color codes
    const BOLD: &str = "\x1b[1m";
    const GREEN: &str = "\x1b[32m";
    const CYAN: &str = "\x1b[36m";
    const YELLOW: &str = "\x1b[33m";
    const RESET: &str = "\x1b[0m";

    println!("{}{}{}", BOLD, "ProtonCLI Query Language Guide", RESET);
    println!("\nThe query language uses Gmail-style syntax to filter messages.\n");

    println!("{}BASIC SYNTAX{}", BOLD, RESET);
    println!("  Query expressions use the format: {}field:value{}\n", CYAN, RESET);
    println!("  {}Examples:{}", BOLD, RESET);
    println!("    protoncli inbox --query \"{}from:user@example.com{}\"", GREEN, RESET);
    println!("    protoncli inbox --query \"{}subject:invoice{}\"\n", GREEN, RESET);

    println!("{}SUPPORTED FIELDS{}\n", BOLD, RESET);

    println!("  {}Sender & Recipients:{}", BOLD, RESET);
    println!("    {}from:{}ADDRESS          Messages from a specific sender", CYAN, RESET);
    println!("    {}to:{}ADDRESS            Messages to a specific recipient\n", CYAN, RESET);

    println!("  {}Content:{}", BOLD, RESET);
    println!("    {}subject:{}TEXT          Search in subject line", CYAN, RESET);
    println!("    {}body:{}TEXT             Search in message body\n", CYAN, RESET);

    println!("  {}Status:{}", BOLD, RESET);
    println!("    {}unread:true{}           Only unread messages", CYAN, RESET);
    println!("    {}is:unread{}             Alternative syntax for unread messages\n", CYAN, RESET);

    println!("  {}Date Filters:{}", BOLD, RESET);
    println!("    {}date:>{}YYYY-MM-DD      Messages after a date", CYAN, RESET);
    println!("    {}date:<{}YYYY-MM-DD      Messages before a date", CYAN, RESET);
    println!("    {}since:{}YYYY-MM-DD      Messages since a date (inclusive)", CYAN, RESET);
    println!("    {}before:{}YYYY-MM-DD     Messages before a date (exclusive)\n", CYAN, RESET);

    println!("  {}Size:{}", BOLD, RESET);
    println!("    {}size:>{}BYTES           Messages larger than size", CYAN, RESET);
    println!("    {}size:<{}BYTES           Messages smaller than size\n", CYAN, RESET);

    println!("{}BOOLEAN OPERATORS{}\n", BOLD, RESET);

    println!("  {}AND{} (both conditions must match):", YELLOW, RESET);
    println!("    from:alice@example.com {}AND{} subject:report", YELLOW, RESET);
    println!("    from:bob@example.com subject:invoice    (implicit AND)\n");

    println!("  {}OR{} (either condition must match):", YELLOW, RESET);
    println!("    from:alice@example.com {}OR{} from:bob@example.com\n", YELLOW, RESET);

    println!("  {}NOT{} (negation):", YELLOW, RESET);
    println!("    {}NOT{} from:spam@example.com", YELLOW, RESET);
    println!("    subject:important {}NOT{} is:unread\n", YELLOW, RESET);

    println!("{}COMPLEX EXAMPLES{}\n", BOLD, RESET);

    println!("  Unread messages from a specific sender:");
    println!("    protoncli inbox --query \"from:support@github.com {}AND{} unread:true\"\n", YELLOW, RESET);

    println!("  Messages from last month:");
    println!("    protoncli inbox --query \"date:>2024-01-01 {}AND{} date:<2024-02-01\"\n", YELLOW, RESET);

    println!("  Important messages, excluding newsletters:");
    println!("    protoncli inbox --query \"subject:urgent {}NOT{} from:newsletter@example.com\"\n", YELLOW, RESET);

    println!("  Large messages from specific domain:");
    println!("    protoncli inbox --query \"from:example.com {}AND{} size:>1000000\"\n", YELLOW, RESET);

    println!("{}COMBINING WITH OTHER FLAGS{}\n", BOLD, RESET);
    println!("  You can combine queries with other inbox flags:\n");
    println!("    {}--limit{} N             Limit results to N messages", CYAN, RESET);
    println!("    {}--output{} FORMAT       Output format (json or markdown)", CYAN, RESET);
    println!("    {}--agent-unread{}        Filter by agent-read status", CYAN, RESET);
    println!("    {}--days{} N              Alternative to date: queries\n", CYAN, RESET);

    println!("  {}Example:{}", BOLD, RESET);
    println!("    protoncli inbox --query \"from:important@example.com\" {}--limit{} 10 {}--output{} markdown\n", CYAN, RESET, CYAN, RESET);

    println!("{}NOTES{}\n", BOLD, RESET);
    println!("  - Field names are case-insensitive (FROM: works same as from:)");
    println!("  - Values are case-sensitive");
    println!("  - Use quotes around values with spaces: subject:\"project update\"");
    println!("  - Date format must be YYYY-MM-DD");
    println!("  - Size is in bytes");
}

