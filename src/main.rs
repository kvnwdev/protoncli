mod cli;
mod core;
mod models;
mod output;
mod utils;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "protoncli")]
#[command(version = "0.1.0")]
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

