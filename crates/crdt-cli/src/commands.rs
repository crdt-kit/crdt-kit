use std::path::Path;

use console::style;
use crdt_migrate::VersionedEnvelope;
use crdt_store::{EventStore, SqliteStore, StateStore};
use inquire::{Confirm, Select, Text};
use serde_json::json;

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

// ── New project scaffolding ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum Platform {
    Cli,
    Dioxus,
    Iot,
    Edge,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::Cli => write!(f, "CLI App          local-first command-line application"),
            Platform::Dioxus => write!(f, "Dioxus Client    cross-platform UI with Dioxus"),
            Platform::Iot => write!(f, "IoT Device       lightweight embedded/IoT node"),
            Platform::Edge => write!(f, "Edge Computing   distributed edge node with sync"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Template {
    Minimal,
    Full,
    Empty,
}

impl std::fmt::Display for Template {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Template::Minimal => write!(f, "Minimal    single entity, basic setup"),
            Template::Full => write!(f, "Full       events + sync, example entities"),
            Template::Empty => write!(f, "Empty      just the skeleton, no entities"),
        }
    }
}

struct ProjectConfig {
    name: String,
    platform: Platform,
    template: Template,
    entity_name: Option<String>,
    enable_events: bool,
    enable_sync: bool,
}

/// `crdt new` — Create a new crdt-kit project with interactive setup.
pub fn new_project(name: Option<String>) -> Result {
    // ── Banner ───────────────────────────────────────────────────────
    println!();
    println!(
        "  {}",
        style("  crdt-kit  ").bold().on_cyan().black()
    );
    println!(
        "  {}",
        style("  Create a new local-first project").dim()
    );
    println!();

    // ── 1. Project name ──────────────────────────────────────────────
    let project_name = match name {
        Some(n) => {
            println!(
                "  {} Project: {}",
                style(">").cyan().bold(),
                style(&n).bold()
            );
            n
        }
        None => Text::new("  Project name:")
            .with_default("my-crdt-app")
            .with_help_message("Used as the crate name and directory")
            .with_validator(|input: &str| {
                if input.is_empty() {
                    return Ok(inquire::validator::Validation::Invalid(
                        "Name cannot be empty".into(),
                    ));
                }
                if !input
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                {
                    return Ok(inquire::validator::Validation::Invalid(
                        "Only alphanumeric, hyphens, and underscores".into(),
                    ));
                }
                if input.starts_with('-') || input.starts_with('_') {
                    return Ok(inquire::validator::Validation::Invalid(
                        "Must start with a letter or number".into(),
                    ));
                }
                Ok(inquire::validator::Validation::Valid)
            })
            .prompt()?,
    };

    // ── 2. Platform ──────────────────────────────────────────────────
    let platforms = vec![Platform::Cli, Platform::Dioxus, Platform::Iot, Platform::Edge];
    let platform = Select::new("  Platform:", platforms)
        .with_help_message("What kind of app are you building?")
        .prompt()?;

    // ── 3. Template ──────────────────────────────────────────────────
    let templates = vec![Template::Minimal, Template::Full, Template::Empty];
    let template = Select::new("  Template:", templates)
        .with_help_message("How much boilerplate to include")
        .prompt()?;

    // ── 4. Starter entity ────────────────────────────────────────────
    let default_entity = match platform {
        Platform::Iot => "SensorReading",
        Platform::Edge => "Document",
        _ => "Task",
    };

    let entity_name = match template {
        Template::Empty => None,
        _ => {
            let name = Text::new("  Starter entity name:")
                .with_default(default_entity)
                .with_help_message("PascalCase name for your first CRDT entity")
                .prompt()?;
            Some(name)
        }
    };

    // ── 5. Features ──────────────────────────────────────────────────
    let (enable_events, enable_sync) = match template {
        Template::Full => (true, true),
        Template::Empty => (false, false),
        Template::Minimal => {
            let events = Confirm::new("  Enable event sourcing?")
                .with_default(platform != Platform::Iot)
                .with_help_message("Append-only event log for audit trails and replay")
                .prompt()?;

            let sync = Confirm::new("  Enable delta sync?")
                .with_default(platform == Platform::Edge || platform == Platform::Dioxus)
                .with_help_message("CRDT delta helpers for conflict-free multi-device sync")
                .prompt()?;

            (events, sync)
        }
    };

    let config = ProjectConfig {
        name: project_name,
        platform,
        template,
        entity_name,
        enable_events,
        enable_sync,
    };

    // ── Summary before generating ────────────────────────────────────
    println!();
    println!("  {}", style("Configuration:").bold());
    println!(
        "    {}  {}",
        style("Project").dim(),
        style(&config.name).cyan()
    );
    println!(
        "    {}  {}",
        style("Platform").dim(),
        style(platform_label(config.platform)).cyan()
    );
    println!(
        "    {}  {}",
        style("Template").dim(),
        style(template_label(config.template)).cyan()
    );
    if let Some(ref e) = config.entity_name {
        println!(
            "    {}  {}",
            style("Entity").dim(),
            style(e).cyan()
        );
    }
    println!(
        "    {}  events={} sync={}",
        style("Features").dim(),
        if config.enable_events {
            style("on").green()
        } else {
            style("off").red()
        },
        if config.enable_sync {
            style("on").green()
        } else {
            style("off").red()
        },
    );
    println!();

    // ── Generate ─────────────────────────────────────────────────────
    println!(
        "  {} Scaffolding project...",
        style("~").cyan().bold()
    );
    println!();

    generate_project(&config)?;

    // ── Done ─────────────────────────────────────────────────────────
    println!();
    println!(
        "  {} {}",
        style("  Done!  ").bold().on_green().black(),
        style("Project created successfully").green()
    );
    println!();
    println!("  Get started:");
    println!();
    println!(
        "    {}  {}",
        style("1.").dim(),
        style(format!("cd {}", config.name)).cyan()
    );

    match config.platform {
        Platform::Dioxus => {
            println!(
                "    {}  {}",
                style("2.").dim(),
                style("dx serve").cyan()
            );
        }
        _ => {
            println!(
                "    {}  {}",
                style("2.").dim(),
                style("cargo run").cyan()
            );
        }
    }

    println!();
    println!(
        "  {}",
        style("  Or use `crdt dev` for the full development runtime with Dev UI dashboard  ").dim()
    );
    println!(
        "  {}",
        style("  Edit crdt-schema.toml to add entities, then run `crdt generate`  ").dim()
    );
    println!();

    Ok(())
}

fn platform_label(p: Platform) -> &'static str {
    match p {
        Platform::Cli => "CLI App",
        Platform::Dioxus => "Dioxus Client",
        Platform::Iot => "IoT Device",
        Platform::Edge => "Edge Computing",
    }
}

fn template_label(t: Template) -> &'static str {
    match t {
        Template::Minimal => "Minimal",
        Template::Full => "Full",
        Template::Empty => "Empty",
    }
}

fn print_step(action: &str, detail: &str) {
    println!(
        "    {} {}",
        style(format!("{action}")).green(),
        detail
    );
}

fn generate_project(config: &ProjectConfig) -> Result {
    let project_dir = Path::new(&config.name);
    if project_dir.exists() {
        return Err(format!("Directory '{}' already exists", config.name).into());
    }

    let src_dir = project_dir.join("src");
    std::fs::create_dir_all(&src_dir)?;

    // Cargo.toml
    let cargo_toml = generate_cargo_toml(&config.name, config.platform);
    std::fs::write(project_dir.join("Cargo.toml"), cargo_toml)?;
    print_step("created", "Cargo.toml");

    // crdt-schema.toml
    let schema_toml = generate_schema_toml(
        config.entity_name.as_deref(),
        config.enable_events,
        config.enable_sync,
        config.template,
        config.platform,
    );
    let schema_path = project_dir.join("crdt-schema.toml");
    std::fs::write(&schema_path, &schema_toml)?;
    print_step("created", "crdt-schema.toml");

    // Run codegen if we have entities
    if config.entity_name.is_some() {
        match crdt_codegen::generate(&schema_path) {
            Ok(result) => {
                let output_dir = project_dir.join(&result.output_dir);
                std::fs::create_dir_all(&output_dir)?;

                for file in &result.files {
                    let full_path = output_dir.join(&file.relative_path);
                    if let Some(parent) = full_path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&full_path, &file.content)?;
                }
                print_step(
                    "generated",
                    &format!("{} files in src/persistence/", result.files.len()),
                );
            }
            Err(e) => {
                println!(
                    "    {} codegen skipped: {}",
                    style("!").yellow(),
                    e
                );
            }
        }
    }

    // src/main.rs
    let main_rs = generate_main_rs(&config.name, config.entity_name.is_some(), config.platform);
    std::fs::write(src_dir.join("main.rs"), main_rs)?;
    print_step("created", "src/main.rs");

    // Platform-specific files
    match config.platform {
        Platform::Dioxus => {
            let dioxus_toml = generate_dioxus_toml(&config.name);
            std::fs::write(project_dir.join("Dioxus.toml"), dioxus_toml)?;
            print_step("created", "Dioxus.toml");
        }
        Platform::Iot => {
            let config_rs = generate_iot_config();
            std::fs::write(src_dir.join("config.rs"), config_rs)?;
            print_step("created", "src/config.rs");
        }
        Platform::Edge => {
            let sync_rs = generate_edge_sync();
            std::fs::write(src_dir.join("sync.rs"), sync_rs)?;
            print_step("created", "src/sync.rs");
        }
        Platform::Cli => {}
    }

    // .gitignore
    let gitignore = match config.platform {
        Platform::Dioxus => "/target\n*.db\n/dist\n/gen\n",
        _ => "/target\n*.db\n",
    };
    std::fs::write(project_dir.join(".gitignore"), gitignore)?;
    print_step("created", ".gitignore");

    Ok(())
}

fn generate_cargo_toml(name: &str, platform: Platform) -> String {
    let mut deps = String::new();
    deps.push_str(&format!(
        "crdt-kit = {{ version = \"0.3.0\", features = [\"serde\"] }}\n"
    ));
    deps.push_str("crdt-store = { version = \"0.2.0\", features = [\"sqlite\"] }\n");
    deps.push_str("crdt-migrate = \"0.2.0\"\n");
    deps.push_str("serde = { version = \"1\", features = [\"derive\"] }\n");
    deps.push_str("postcard = { version = \"1\", features = [\"alloc\"] }\n");

    match platform {
        Platform::Dioxus => {
            deps.push_str("dioxus = { version = \"0.6\", features = [\"desktop\"] }\n");
        }
        Platform::Iot => {
            deps.push_str("tokio = { version = \"1\", features = [\"rt\", \"macros\", \"time\"] }\n");
            deps.push_str("tracing = \"0.1\"\n");
            deps.push_str("tracing-subscriber = \"0.3\"\n");
        }
        Platform::Edge => {
            deps.push_str(
                "tokio = { version = \"1\", features = [\"rt-multi-thread\", \"macros\", \"net\", \"time\"] }\n",
            );
            deps.push_str("tracing = \"0.1\"\n");
            deps.push_str("tracing-subscriber = \"0.3\"\n");
        }
        Platform::Cli => {}
    }

    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[workspace]

[[bin]]
name = "{name}"
path = "src/main.rs"

[dependencies]
{deps}"#
    )
}

fn generate_schema_toml(
    entity_name: Option<&str>,
    enable_events: bool,
    enable_sync: bool,
    template: Template,
    platform: Platform,
) -> String {
    let mut buf = String::new();

    buf.push_str("[config]\noutput = \"src/persistence\"\n\n");
    buf.push_str(&format!(
        "[config.events]\nenabled = {enable_events}\nsnapshot_threshold = 100\n\n"
    ));
    buf.push_str(&format!("[config.sync]\nenabled = {enable_sync}\n"));

    if let Some(name) = entity_name {
        let table = to_snake_plural(name);
        buf.push_str(&format!(
            "\n[[entity]]\nname = \"{name}\"\ntable = \"{table}\"\n"
        ));
        buf.push_str("\n[[entity.versions]]\nversion = 1\nfields = [\n");

        match (template, platform) {
            (Template::Full, Platform::Iot) => {
                buf.push_str(
                    "    { name = \"device_id\", type = \"String\" },\n",
                );
                buf.push_str(
                    "    { name = \"value\", type = \"f64\" },\n",
                );
                buf.push_str(
                    "    { name = \"unit\", type = \"String\" },\n",
                );
                buf.push_str(
                    "    { name = \"timestamp\", type = \"u64\" },\n",
                );
                buf.push_str(
                    "    { name = \"readings\", type = \"f64\", crdt = \"GCounter\" },\n",
                );
            }
            (_, Platform::Iot) => {
                buf.push_str(
                    "    { name = \"device_id\", type = \"String\" },\n",
                );
                buf.push_str(
                    "    { name = \"value\", type = \"f64\" },\n",
                );
                buf.push_str(
                    "    { name = \"unit\", type = \"String\" },\n",
                );
                buf.push_str(
                    "    { name = \"timestamp\", type = \"u64\" },\n",
                );
            }
            (Template::Full, Platform::Edge) => {
                buf.push_str(
                    "    { name = \"title\", type = \"String\", crdt = \"LWWRegister\" },\n",
                );
                buf.push_str(
                    "    { name = \"body\", type = \"String\", crdt = \"LWWRegister\" },\n",
                );
                buf.push_str(
                    "    { name = \"tags\", type = \"String\", crdt = \"ORSet\" },\n",
                );
                buf.push_str(
                    "    { name = \"version\", type = \"u64\", crdt = \"GCounter\" },\n",
                );
            }
            (_, Platform::Edge) => {
                buf.push_str(
                    "    { name = \"title\", type = \"String\" },\n",
                );
                buf.push_str(
                    "    { name = \"body\", type = \"String\" },\n",
                );
            }
            (Template::Full, Platform::Dioxus) => {
                buf.push_str(
                    "    { name = \"title\", type = \"String\", crdt = \"LWWRegister\" },\n",
                );
                buf.push_str(
                    "    { name = \"done\", type = \"bool\" },\n",
                );
                buf.push_str(
                    "    { name = \"tags\", type = \"String\", crdt = \"ORSet\" },\n",
                );
            }
            (Template::Full, _) => {
                buf.push_str(
                    "    { name = \"title\", type = \"String\", crdt = \"LWWRegister\" },\n",
                );
                buf.push_str(
                    "    { name = \"done\", type = \"bool\" },\n",
                );
                buf.push_str(
                    "    { name = \"tags\", type = \"String\", crdt = \"ORSet\" },\n",
                );
            }
            _ => {
                buf.push_str(
                    "    { name = \"title\", type = \"String\" },\n",
                );
                buf.push_str(
                    "    { name = \"done\", type = \"bool\" },\n",
                );
            }
        }
        buf.push_str("]\n");
    }

    buf
}

fn generate_main_rs(project_name: &str, has_entities: bool, platform: Platform) -> String {
    match platform {
        Platform::Dioxus => generate_dioxus_main(project_name, has_entities),
        Platform::Iot => generate_iot_main(project_name, has_entities),
        Platform::Edge => generate_edge_main(project_name, has_entities),
        Platform::Cli => generate_cli_main(project_name, has_entities),
    }
}

fn generate_cli_main(project_name: &str, has_entities: bool) -> String {
    if has_entities {
        format!(
            r#"mod persistence;

use crdt_store::SqliteStore;
use persistence::Persistence;

fn main() {{
    let store = SqliteStore::open("{project_name}.db").expect("Failed to open database");
    let _db = Persistence::new(store);

    println!("  {project_name} is running");
    println!("  Database: {project_name}.db");

    // Example: access the generated repository
    // let mut db = Persistence::new(store);
    // let mut repo = db.tasks();
    // repo.save("task-1", &my_task).unwrap();

    println!("  Ready! Edit src/main.rs to start building.");
}}
"#
        )
    } else {
        format!(
            r#"use crdt_store::SqliteStore;

fn main() {{
    let _store = SqliteStore::open("{project_name}.db").expect("Failed to open database");

    println!("  {project_name} is running");
    println!("  Database: {project_name}.db");
}}
"#
        )
    }
}

fn generate_dioxus_main(project_name: &str, has_entities: bool) -> String {
    if has_entities {
        format!(
            r#"mod persistence;

use dioxus::prelude::*;
use crdt_store::SqliteStore;
use persistence::Persistence;

fn main() {{
    dioxus::launch(app);
}}

fn app() -> Element {{
    let store = SqliteStore::open("{project_name}.db").expect("Failed to open database");
    let _db = Persistence::new(store);

    rsx! {{
        div {{ class: "container",
            h1 {{ "{project_name}" }}
            p {{ "Local-first app powered by crdt-kit" }}
        }}
    }}
}}
"#
        )
    } else {
        format!(
            r#"use dioxus::prelude::*;

fn main() {{
    dioxus::launch(app);
}}

fn app() -> Element {{
    rsx! {{
        div {{ class: "container",
            h1 {{ "{project_name}" }}
            p {{ "Local-first app powered by crdt-kit" }}
        }}
    }}
}}
"#
        )
    }
}

fn generate_iot_main(project_name: &str, has_entities: bool) -> String {
    let persistence_mod = if has_entities { "mod persistence;\nmod config;\n" } else { "mod config;\n" };
    let db_init = if has_entities {
        format!(
            r#"    let store = crdt_store::SqliteStore::open("{project_name}.db")?;
    let mut _db = persistence::Persistence::new(store);
    tracing::info!("database ready: {project_name}.db");"#
        )
    } else {
        format!(
            r#"    let _store = crdt_store::SqliteStore::open("{project_name}.db")?;
    tracing::info!("database ready: {project_name}.db");"#
        )
    };

    format!(
        r#"{persistence_mod}
#[tokio::main(flavor = "current_thread")]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {{
    tracing_subscriber::fmt::init();
    tracing::info!("{project_name} IoT node starting...");

{db_init}

    // Main sensor loop
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
    loop {{
        interval.tick().await;
        tracing::info!("tick - reading sensors...");
        // TODO: read sensor data and store via db
    }}
}}
"#
    )
}

fn generate_edge_main(project_name: &str, has_entities: bool) -> String {
    let persistence_mod = if has_entities { "mod persistence;\nmod sync;\n" } else { "mod sync;\n" };
    let db_init = if has_entities {
        format!(
            r#"    let store = crdt_store::SqliteStore::open("{project_name}.db")?;
    let mut _db = persistence::Persistence::new(store);
    tracing::info!("database ready: {project_name}.db");"#
        )
    } else {
        format!(
            r#"    let _store = crdt_store::SqliteStore::open("{project_name}.db")?;
    tracing::info!("database ready: {project_name}.db");"#
        )
    };

    format!(
        r#"{persistence_mod}
#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {{
    tracing_subscriber::fmt::init();
    tracing::info!("{project_name} edge node starting...");

{db_init}

    // TODO: start sync listener
    let addr = "0.0.0.0:4001";
    tracing::info!("listening for peers on {{addr}}");

    // Keep running
    tokio::signal::ctrl_c().await?;
    tracing::info!("shutting down");
    Ok(())
}}
"#
    )
}

fn generate_dioxus_toml(name: &str) -> String {
    format!(
        r#"[application]
name = "{name}"
default_platform = "desktop"

[web.app]
title = "{name}"

[web.watcher]
reload_html = true
watch_path = ["src"]
"#
    )
}

fn generate_iot_config() -> String {
    r#"/// IoT device configuration.

pub struct DeviceConfig {
    pub device_id: String,
    pub read_interval_secs: u64,
    pub db_path: String,
}

impl Default for DeviceConfig {
    fn default() -> Self {
        Self {
            device_id: format!("node-{:04x}", rand_id()),
            read_interval_secs: 5,
            db_path: "device.db".to_string(),
        }
    }
}

fn rand_id() -> u16 {
    use std::time::SystemTime;
    let t = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (t & 0xFFFF) as u16
}
"#
    .to_string()
}

fn generate_edge_sync() -> String {
    r#"/// Placeholder for edge sync logic.
///
/// In a real deployment, this module would handle:
/// - Peer discovery (mDNS, gossip, or manual config)
/// - Delta exchange over TCP/UDP
/// - Merge of incoming CRDT states

pub struct SyncConfig {
    pub listen_addr: String,
    pub peer_addrs: Vec<String>,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:4001".to_string(),
            peer_addrs: Vec::new(),
        }
    }
}
"#
    .to_string()
}

fn to_snake_plural(name: &str) -> String {
    let mut result = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(ch.to_ascii_lowercase());
    }
    if result.ends_with('s') {
        result.push_str("es");
    } else {
        result.push('s');
    }
    result
}

// ── Database commands ────────────────────────────────────────────────

/// `crdt status <db>` — Show database status and statistics.
pub fn status(db_path: &str) -> Result {
    let store = SqliteStore::open(db_path)?;
    let info = store.db_info()?;
    let size = store.file_size()?;
    let journal = store.journal_mode()?;

    println!("Database: {db_path} (SQLite, {journal} mode)");
    println!("Size: {}", format_bytes(size));
    println!();

    if info.namespaces.is_empty() {
        println!("  (empty database)");
        return Ok(());
    }

    // Header
    println!(
        "  {:<20} {:>10} {:>10} {:>10}",
        "Namespace", "Entities", "Events", "Snapshots"
    );
    println!("  {}", "-".repeat(54));

    for ns in &info.namespaces {
        println!(
            "  {:<20} {:>10} {:>10} {:>10}",
            ns.name,
            format_num(ns.entity_count),
            format_num(ns.event_count),
            format_num(ns.snapshot_count),
        );
    }

    println!("  {}", "-".repeat(54));
    println!(
        "  {:<20} {:>10} {:>10} {:>10}",
        "Total",
        format_num(info.total_entities),
        format_num(info.total_events),
        format_num(info.total_snapshots),
    );
    println!();

    Ok(())
}

/// `crdt inspect <db> [key]` — Inspect entities or list namespaces.
pub fn inspect(
    db_path: &str,
    key: Option<&str>,
    namespace: Option<&str>,
    show_events: bool,
    last: usize,
) -> Result {
    let store = SqliteStore::open(db_path)?;

    match key {
        Some(key) => inspect_entity(&store, key, namespace, show_events, last),
        None => inspect_list(&store, namespace),
    }
}

fn inspect_entity(
    store: &SqliteStore,
    key: &str,
    namespace: Option<&str>,
    show_events: bool,
    last: usize,
) -> Result {
    // If namespace given, look there. Otherwise search all namespaces.
    let namespaces: Vec<String> = if let Some(ns) = namespace {
        vec![ns.to_string()]
    } else {
        let info = store.db_info()?;
        info.namespaces.into_iter().map(|n| n.name).collect()
    };

    let mut found = false;

    for ns in &namespaces {
        if let Some(data) = store.get(ns, key)? {
            found = true;
            println!("Entity: {key}");
            println!("Namespace: {ns}");
            println!("Size: {} bytes", data.len());

            if VersionedEnvelope::is_versioned(&data) {
                if let Ok(env) = VersionedEnvelope::from_bytes(&data) {
                    println!("Version: v{}", env.version);
                    println!("CRDT type: {:?}", env.crdt_type);
                    println!("Payload: {} bytes", env.payload.len());
                }
            } else {
                println!("Version: (unversioned)");
            }
            println!();

            if show_events {
                inspect_events(store, ns, key, last)?;
            }
            break;
        }
    }

    if !found {
        eprintln!("Entity '{key}' not found");
    }

    Ok(())
}

fn inspect_events(store: &SqliteStore, namespace: &str, entity_id: &str, last: usize) -> Result {
    let all_events = store.events_since(namespace, entity_id, 0)?;
    let total = all_events.len();

    if total == 0 {
        println!("  (no events)");
        return Ok(());
    }

    let events: Vec<_> = if total > last {
        all_events[total - last..].to_vec()
    } else {
        all_events
    };

    println!(
        "Event log ({} of {} events):",
        events.len(),
        format_num(total as u64)
    );
    println!(
        "  {:>8}  {:>14}  {:<16}  {:>10}",
        "Seq", "Timestamp", "Node", "Size"
    );
    println!("  {}", "-".repeat(56));

    for ev in &events {
        println!(
            "  {:>8}  {:>14}  {:<16}  {:>7} B",
            ev.sequence,
            ev.timestamp,
            truncate(&ev.node_id, 16),
            ev.data.len(),
        );
    }

    // Snapshot info
    if let Some(snap) = store.load_snapshot(namespace, entity_id)? {
        println!();
        println!(
            "Snapshot: at seq {}, v{}, {} bytes",
            snap.at_sequence,
            snap.version,
            snap.state.len()
        );
    }

    println!();
    Ok(())
}

fn inspect_list(store: &SqliteStore, namespace: Option<&str>) -> Result {
    let info = store.db_info()?;

    if info.namespaces.is_empty() {
        println!("  (empty database)");
        return Ok(());
    }

    for ns in &info.namespaces {
        if let Some(filter) = namespace {
            if ns.name != filter {
                continue;
            }
        }

        println!("Namespace: {} ({} entities)", ns.name, ns.entity_count);

        let keys = store.list_keys(&ns.name)?;
        for key in &keys {
            let size = store.get(&ns.name, key)?.map(|d| d.len()).unwrap_or(0);
            println!("  {key:<40} {size:>8} B");
        }
        println!();
    }

    Ok(())
}

/// `crdt compact <db>` — Compact event logs.
pub fn compact(db_path: &str, namespace: Option<&str>, threshold: u64) -> Result {
    let mut store = SqliteStore::open(db_path)?;
    let info = store.db_info()?;

    let mut total_removed = 0u64;
    let mut compacted = 0u64;

    for ns in &info.namespaces {
        if let Some(filter) = namespace {
            if ns.name != filter {
                continue;
            }
        }

        // Get all entity IDs in this namespace that have events
        let keys = store.list_keys(&ns.name)?;

        for key in &keys {
            let count = store.event_count(&ns.name, key)?;
            if count < threshold {
                continue;
            }

            // Get current state as snapshot data
            let state = store.get(&ns.name, key)?.unwrap_or_default();

            // Get max sequence
            let events = store.events_since(&ns.name, key, 0)?;
            let max_seq = events.last().map(|e| e.sequence).unwrap_or(0);

            if max_seq == 0 {
                continue;
            }

            // Determine version from envelope
            let version = if VersionedEnvelope::is_versioned(&state) {
                VersionedEnvelope::peek_version(&state).unwrap_or(1)
            } else {
                1
            };

            // Save snapshot
            store.save_snapshot(&ns.name, key, &state, max_seq, version)?;

            // Truncate events before max_seq (keep latest)
            let removed = store.truncate_events_before(&ns.name, key, max_seq)?;

            if removed > 0 {
                println!(
                    "  {}/{}: {} events -> snapshot + {} events",
                    ns.name,
                    key,
                    format_num(count),
                    format_num(count - removed)
                );
                total_removed += removed;
                compacted += 1;
            }
        }
    }

    if compacted == 0 {
        println!("Nothing to compact (threshold: {threshold} events)");
    } else {
        println!();
        println!(
            "Compacted {compacted} entities, removed {} events",
            format_num(total_removed)
        );
    }

    Ok(())
}

/// `crdt export <db>` — Export data as JSON.
pub fn export(db_path: &str, key: Option<&str>, namespace: Option<&str>) -> Result {
    let store = SqliteStore::open(db_path)?;

    match key {
        Some(key) => export_entity(&store, key, namespace),
        None => export_namespace(&store, namespace),
    }
}

fn export_entity(store: &SqliteStore, key: &str, namespace: Option<&str>) -> Result {
    let namespaces: Vec<String> = if let Some(ns) = namespace {
        vec![ns.to_string()]
    } else {
        let info = store.db_info()?;
        info.namespaces.into_iter().map(|n| n.name).collect()
    };

    for ns in &namespaces {
        if let Some(data) = store.get(ns, key)? {
            let entry = if VersionedEnvelope::is_versioned(&data) {
                let env = VersionedEnvelope::from_bytes(&data)
                    .map_err(|e| format!("envelope error: {e}"))?;
                json!({
                    "namespace": ns,
                    "key": key,
                    "version": env.version,
                    "crdt_type": format!("{:?}", env.crdt_type),
                    "payload_size": env.payload.len(),
                    "payload_base64": base64_encode(&env.payload),
                })
            } else {
                json!({
                    "namespace": ns,
                    "key": key,
                    "version": null,
                    "raw_size": data.len(),
                    "raw_base64": base64_encode(&data),
                })
            };

            // Export events too
            let events = store.events_since(ns, key, 0)?;
            let events_json: Vec<_> = events
                .iter()
                .map(|e| {
                    json!({
                        "sequence": e.sequence,
                        "timestamp": e.timestamp,
                        "node_id": e.node_id,
                        "data_size": e.data.len(),
                        "data_base64": base64_encode(&e.data),
                    })
                })
                .collect();

            let output = json!({
                "entity": entry,
                "events": events_json,
            });

            println!("{}", serde_json::to_string_pretty(&output)?);
            return Ok(());
        }
    }

    eprintln!("Entity '{key}' not found");
    Ok(())
}

fn export_namespace(store: &SqliteStore, namespace: Option<&str>) -> Result {
    let info = store.db_info()?;
    let mut entries = Vec::new();

    for ns in &info.namespaces {
        if let Some(filter) = namespace {
            if ns.name != filter {
                continue;
            }
        }

        let keys = store.list_keys(&ns.name)?;
        for key in &keys {
            if let Some(data) = store.get(&ns.name, key)? {
                let entry = if VersionedEnvelope::is_versioned(&data) {
                    let env = VersionedEnvelope::from_bytes(&data)
                        .map_err(|e| format!("envelope error: {e}"))?;
                    json!({
                        "namespace": ns.name,
                        "key": key,
                        "version": env.version,
                        "crdt_type": format!("{:?}", env.crdt_type),
                        "payload_size": env.payload.len(),
                    })
                } else {
                    json!({
                        "namespace": ns.name,
                        "key": key,
                        "raw_size": data.len(),
                    })
                };
                entries.push(entry);
            }
        }
    }

    let output = json!({
        "database": {
            "total_entities": info.total_entities,
            "total_events": info.total_events,
            "total_snapshots": info.total_snapshots,
        },
        "entries": entries,
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

/// `crdt generate` — Generate Rust code from a schema definition.
pub fn generate(schema_path: &str, output_override: Option<&str>, dry_run: bool) -> Result {
    let path = std::path::Path::new(schema_path);

    if !path.exists() {
        return Err(format!("Schema file not found: {schema_path}").into());
    }

    let result =
        crdt_codegen::generate(path).map_err(|e| format!("Code generation failed: {e}"))?;

    let output_dir = output_override.unwrap_or(&result.output_dir);

    if dry_run {
        println!(
            "Dry run — would generate {} files in {output_dir}/:\n",
            result.files.len()
        );
        for file in &result.files {
            println!("--- {} ---", file.relative_path);
            println!("{}", file.content);
        }
        return Ok(());
    }

    std::fs::create_dir_all(output_dir)?;

    for file in &result.files {
        let full_path = std::path::Path::new(output_dir).join(&file.relative_path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full_path, &file.content)?;
        println!("  Generated: {}", full_path.display());
    }

    println!("\nGenerated {} files in {output_dir}/", result.files.len());
    Ok(())
}

// ── Dev runtime ──────────────────────────────────────────────────────

fn timestamp() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

fn log_dev(msg: &str) {
    println!(
        "  {} {} {}",
        style(timestamp()).dim(),
        style("[crdt]").cyan().bold(),
        msg
    );
}

fn log_app(msg: &str) {
    println!(
        "  {} {} {}",
        style(timestamp()).dim(),
        style("[app]").magenta(),
        msg
    );
}

fn log_app_err(msg: &str) {
    eprintln!(
        "  {} {} {}",
        style(timestamp()).dim(),
        style("[app]").yellow(),
        msg
    );
}

fn log_ui(msg: &str) {
    println!(
        "  {} {} {}",
        style(timestamp()).dim(),
        style("[ui]").blue(),
        msg
    );
}

/// Auto-detect the DB path from crdt-schema.toml or Cargo.toml project name.
fn detect_db_path(schema_path: &str) -> String {
    // Try to get project name from Cargo.toml
    if let Ok(content) = std::fs::read_to_string("Cargo.toml") {
        for line in content.lines() {
            if let Some(name) = line.strip_prefix("name") {
                let name = name.trim().trim_start_matches('=').trim().trim_matches('"');
                if !name.is_empty() {
                    return format!("{name}.db");
                }
            }
        }
    }
    // Fallback: use schema filename as hint
    let _ = schema_path;
    "app.db".to_string()
}

/// Auto-detect crdt-schema.toml location.
fn detect_schema_path() -> Option<String> {
    let candidates = ["crdt-schema.toml", "schema.toml"];
    for c in &candidates {
        if Path::new(c).exists() {
            return Some(c.to_string());
        }
    }
    None
}

/// Run codegen from schema if it exists, returns number of files generated.
fn run_codegen(schema_path: &str) -> std::result::Result<usize, String> {
    let path = Path::new(schema_path);
    if !path.exists() {
        return Err(format!("Schema not found: {schema_path}"));
    }

    let result =
        crdt_codegen::generate(path).map_err(|e| format!("Codegen failed: {e}"))?;

    let output_dir = &result.output_dir;
    std::fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;

    for file in &result.files {
        let full_path = Path::new(output_dir).join(&file.relative_path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&full_path, &file.content).map_err(|e| e.to_string())?;
    }

    Ok(result.files.len())
}

/// Get file modification timestamp for change detection.
fn file_mtime(path: &str) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

/// `crdt dev` — Development runtime: run app + Dev UI dashboard in parallel.
pub async fn dev_server(
    db_path: Option<&str>,
    port: u16,
    cargo_cmd: &str,
    watch: bool,
    open_browser: bool,
    schema_override: Option<&str>,
) -> Result {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    // ── Auto-detect project settings ─────────────────────────────────
    let schema_path = schema_override
        .map(String::from)
        .or_else(detect_schema_path);

    let db = match db_path {
        Some(p) => p.to_string(),
        None => match &schema_path {
            Some(s) => detect_db_path(s),
            None => "app.db".to_string(),
        },
    };

    let url = format!("http://localhost:{port}");

    // ── Banner ───────────────────────────────────────────────────────
    println!();
    println!(
        "  {}{}{}",
        style("  ").on_magenta(),
        style(" crdt dev ").bold().on_magenta().black(),
        style("  ").on_magenta()
    );
    println!();

    // Status table
    println!(
        "  {}     {}",
        style("App").dim(),
        style(format!("cargo {cargo_cmd}")).white().bold()
    );
    println!(
        "  {}  {}",
        style("Dev UI").dim(),
        style(&url).cyan().underlined()
    );
    println!(
        "  {}      {}",
        style("DB").dim(),
        style(&db).white()
    );
    if let Some(ref s) = schema_path {
        println!(
            "  {}  {}",
            style("Schema").dim(),
            style(s).white()
        );
    }
    println!(
        "  {}   {}",
        style("Watch").dim(),
        if watch {
            style("on — auto-restart on exit").green()
        } else {
            style("off").red()
        }
    );
    println!();
    println!(
        "  {}",
        style("─".repeat(52)).dim()
    );
    println!();

    // ── Codegen on start ─────────────────────────────────────────────
    if let Some(ref sp) = schema_path {
        log_dev("Running initial codegen...");
        match run_codegen(sp) {
            Ok(n) => log_dev(&format!(
                "{}",
                style(format!("Generated {n} files from schema")).green()
            )),
            Err(e) => log_dev(&format!(
                "{}",
                style(format!("Codegen warning: {e}")).yellow()
            )),
        }
        println!();
    }

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    // Handle Ctrl+C
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        r.store(false, Ordering::SeqCst);
    });

    // ── Launch Dev UI ────────────────────────────────────────────────
    log_ui(&format!("Starting dashboard on {}", style(&url).cyan()));
    let db_owned = db.clone();
    let ui_handle = tokio::spawn(async move {
        if let Err(e) = crdt_dev_ui::start(&db_owned, port).await {
            eprintln!(
                "  {} {} Dev UI error: {}",
                style(timestamp()).dim(),
                style("[ui]").red().bold(),
                e
            );
        }
    });

    // Open browser after a small delay
    if open_browser {
        let url_clone = url.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            if let Err(e) = open::that(&url_clone) {
                eprintln!(
                    "  {} {} Could not open browser: {}",
                    style(timestamp()).dim(),
                    style("[ui]").yellow(),
                    e
                );
            }
        });
    }

    // ── Build cargo command args ─────────────────────────────────────
    let cmd_parts: Vec<&str> = cargo_cmd.split_whitespace().collect();
    let cargo_args = if cmd_parts.is_empty() {
        vec!["run"]
    } else {
        cmd_parts
    };

    let mut schema_mtime = schema_path.as_deref().and_then(file_mtime);
    let mut run_count = 0u32;

    // ── Main loop ────────────────────────────────────────────────────
    loop {
        run_count += 1;

        // Check for schema changes before each run (except first)
        if run_count > 1 {
            if let Some(ref sp) = schema_path {
                let new_mtime = file_mtime(sp);
                if new_mtime != schema_mtime {
                    log_dev(&format!(
                        "{}",
                        style("Schema changed, regenerating...").yellow()
                    ));
                    match run_codegen(sp) {
                        Ok(n) => log_dev(&format!(
                            "{}",
                            style(format!("Regenerated {n} files")).green()
                        )),
                        Err(e) => log_dev(&format!(
                            "{}",
                            style(format!("Codegen error: {e}")).red()
                        )),
                    }
                    schema_mtime = new_mtime;
                }
            }
        }

        log_app(&format!(
            "{}",
            style(format!("cargo {}", cargo_args.join(" "))).white().bold()
        ));

        let mut child = Command::new("cargo")
            .args(&cargo_args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn cargo: {e}"))?;

        // Stream stdout with timestamps
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let stdout_handle = std::thread::spawn(move || {
            if let Some(out) = stdout {
                let reader = BufReader::new(out);
                for line in reader.lines().map_while(|l| l.ok()) {
                    log_app(&line);
                }
            }
        });

        let stderr_handle = std::thread::spawn(move || {
            if let Some(err) = stderr {
                let reader = BufReader::new(err);
                for line in reader.lines().map_while(|l| l.ok()) {
                    log_app_err(&line);
                }
            }
        });

        let exit_status = child.wait()?;
        stdout_handle.join().ok();
        stderr_handle.join().ok();

        if !running.load(Ordering::SeqCst) {
            break;
        }

        println!();
        if exit_status.success() {
            log_app(&format!(
                "{}",
                style("Process exited (0)").green()
            ));
        } else {
            log_app(&format!(
                "{}",
                style(format!("Process exited with {exit_status}")).red().bold()
            ));
        }

        if !watch {
            // In non-watch mode, show DB stats then keep Dev UI alive
            if Path::new(&db).exists() {
                println!();
                log_dev("Database snapshot:");
                let _ = status(&db);
            }
            println!();
            log_ui(&format!(
                "Dev UI still running at {}",
                style(format!("http://localhost:{port}")).cyan().underlined()
            ));
            log_dev(&format!(
                "Press {} to stop",
                style("Ctrl+C").yellow().bold()
            ));

            // Wait for Ctrl+C while keeping Dev UI alive
            tokio::signal::ctrl_c().await.ok();
            break;
        }

        // Watch mode: wait and restart
        println!();
        log_dev(&format!(
            "Waiting for next run... {}",
            style("(Ctrl+C to stop)").dim()
        ));
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        if !running.load(Ordering::SeqCst) {
            break;
        }

        println!();
        println!(
            "  {}",
            style("─".repeat(52)).dim()
        );
        log_dev(&format!(
            "{}",
            style(format!("Restarting (run #{run_count})...")).cyan()
        ));
        println!();
    }

    // ── Shutdown ─────────────────────────────────────────────────────
    ui_handle.abort();

    println!();
    println!(
        "  {}{}{}",
        style("  ").on_green(),
        style(" Stopped ").bold().on_green().black(),
        style("  ").on_green()
    );
    println!();

    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn format_num(n: u64) -> String {
    if n < 1000 {
        return n.to_string();
    }
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}

fn base64_encode(data: &[u8]) -> String {
    // Simple base64 without pulling in a dependency
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        result.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}
