mod index;
mod page;
mod scanner;
mod server;
mod types;
mod watcher;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use tokio::sync::broadcast;

use crate::index::ImageIndex;
use crate::scanner::{build_deny_set, scan_directory};
use crate::server::{build_router, AppState};

#[derive(Parser)]
#[command(name = "watchpix", version, about = "A live-reloading image gallery server for remote and headless machines")]
struct Cli {
    /// Directory to watch
    #[arg(default_value = ".")]
    root: PathBuf,

    /// Port to listen on
    #[arg(short, long, default_value = "8080")]
    port: u16,

    /// Address to bind to
    #[arg(short, long, default_value = "127.0.0.1")]
    bind: String,

    /// Additional directories to deny, comma-separated
    #[arg(short, long, value_delimiter = ',')]
    deny: Vec<String>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Set up tracing
    let filter = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    // Validate root directory
    let root = match std::fs::canonicalize(&cli.root) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "Error: cannot access directory '{}': {}",
                cli.root.display(),
                e
            );
            std::process::exit(1);
        }
    };

    if !root.is_dir() {
        eprintln!("Error: '{}' is not a directory", root.display());
        std::process::exit(1);
    }

    // Build deny set
    let deny_set = build_deny_set(&cli.deny);

    // Initial scan
    let entries = scan_directory(&root, &root, &deny_set);
    let image_count = entries.len();
    let index = Arc::new(ImageIndex::new(entries));

    // Broadcast channel
    let (tx, _rx) = broadcast::channel::<String>(1024);

    // Start file watcher
    if let Err(e) = watcher::start_watcher(root.clone(), index.clone(), tx.clone(), deny_set.clone()) {
        eprintln!("Error: failed to start file watcher: {}", e);
        if e.to_string().contains("inotify") {
            eprintln!("Tip: increase inotify watch limit with:");
            eprintln!("  echo 65536 | sudo tee /proc/sys/fs/inotify/max_user_watches");
        }
        std::process::exit(1);
    }

    // Build router
    let state = AppState {
        index,
        root: root.clone(),
        tx,
    };
    let app = build_router(state);

    // Bind listener
    let addr: SocketAddr = format!("{}:{}", cli.bind, cli.port)
        .parse()
        .unwrap_or_else(|e| {
            eprintln!("Error: invalid bind address '{}:{}': {}", cli.bind, cli.port, e);
            std::process::exit(1);
        });

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                eprintln!("Error: port {} is already in use", cli.port);
                eprintln!("Tip: try a different port with --port <PORT>");
            } else {
                eprintln!("Error: cannot bind to {}: {}", addr, e);
            }
            std::process::exit(1);
        }
    };

    // Startup banner
    let deny_count = deny_set.len();
    println!("watchpix v{}", env!("CARGO_PKG_VERSION"));
    println!("Watching: {}", root.display());
    println!("Found: {} images", image_count);
    println!("Denying: {} patterns", deny_count);
    println!("Server: http://{}", addr);
    println!();
    println!("Tip: ssh -L {}:localhost:{} user@host", cli.port, cli.port);

    // Serve
    axum::serve(listener, app).await.unwrap();
}
