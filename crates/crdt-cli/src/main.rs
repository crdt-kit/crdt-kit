use std::process;

use clap::{Parser, Subcommand};

mod commands;

/// crdt-cli: Development tool for crdt-kit databases.
///
/// Inspect, migrate, compact, and export CRDT databases from the command line.
#[derive(Parser)]
#[command(name = "crdt", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show database status and statistics.
    Status {
        /// Path to the SQLite database file.
        db: String,
    },

    /// Inspect entities, namespaces, or event logs.
    Inspect {
        /// Path to the SQLite database file.
        db: String,

        /// Entity key to inspect. If omitted, lists all namespaces.
        key: Option<String>,

        /// Namespace to scope the lookup.
        #[arg(short, long)]
        namespace: Option<String>,

        /// Show event log for the entity.
        #[arg(long)]
        events: bool,

        /// Limit the number of events shown.
        #[arg(long, default_value = "20")]
        last: usize,
    },

    /// Compact event logs by creating snapshots and truncating old events.
    Compact {
        /// Path to the SQLite database file.
        db: String,

        /// Only compact a specific namespace.
        #[arg(short, long)]
        namespace: Option<String>,

        /// Minimum number of events before compaction triggers.
        #[arg(long, default_value = "100")]
        threshold: u64,
    },

    /// Export data as JSON for debugging.
    Export {
        /// Path to the SQLite database file.
        db: String,

        /// Entity key to export. If omitted, exports all in the namespace.
        key: Option<String>,

        /// Namespace to export from. Defaults to all.
        #[arg(short, long)]
        namespace: Option<String>,
    },

    /// Launch the Dev UI web panel for visual database inspection.
    DevUi {
        /// Path to the SQLite database file.
        db: String,

        /// Port to serve the Dev UI on.
        #[arg(short, long, default_value = "4242")]
        port: u16,
    },

    /// Generate Rust code from a crdt-schema.toml definition.
    Generate {
        /// Path to the crdt-schema.toml file.
        #[arg(short, long, default_value = "crdt-schema.toml")]
        schema: String,

        /// Override the output directory (ignores the one in the schema file).
        #[arg(short, long)]
        output: Option<String>,

        /// Preview generated files without writing them to disk.
        #[arg(long)]
        dry_run: bool,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result: Result<(), Box<dyn std::error::Error>> = match cli.command {
        Commands::Status { db } => commands::status(&db),
        Commands::Inspect {
            db,
            key,
            namespace,
            events,
            last,
        } => commands::inspect(&db, key.as_deref(), namespace.as_deref(), events, last),
        Commands::Compact {
            db,
            namespace,
            threshold,
        } => commands::compact(&db, namespace.as_deref(), threshold),
        Commands::Export { db, key, namespace } => {
            commands::export(&db, key.as_deref(), namespace.as_deref())
        }
        Commands::DevUi { db, port } => crdt_dev_ui::start(&db, port).await,
        Commands::Generate {
            schema,
            output,
            dry_run,
        } => commands::generate(&schema, output.as_deref(), dry_run),
    };

    if let Err(e) = result {
        eprintln!("error: {e}");
        process::exit(1);
    }
}
