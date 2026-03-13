//! mockingbird CLI.

use chrono::{Duration, Utc};
use clap::Parser;
use mockingbird::cassette::{self, save_cassette};
use mockingbird::error::Result;
use mockingbird::mode::Mode;
use mockingbird::{run_proxy, ProxyConfig};

/// mockingbird - HTTP request recorder and replayer.
#[derive(Parser, Debug)]
#[command(name = "mockingbird")]
#[command(version, about, long_about = None)]
struct Args {
    /// Command to run.
    #[command(subcommand)]
    command: Option<Command>,
}

/// Available commands.
#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Record HTTP interactions to a cassette.
    Record {
        /// Port to listen on.
        #[arg(short, long, default_value = "8080")]
        port: u16,
        
        /// Cassette file to save to.
        #[arg(short, long)]
        cassette: String,
        
        /// Target URL to proxy to.
        #[arg(short, long)]
        target: String,
    },
    
    /// Replay HTTP interactions from a cassette.
    Replay {
        /// Port to listen on.
        #[arg(short, long, default_value = "8080")]
        port: u16,
        
        /// Cassette file to replay from.
        #[arg(short, long)]
        cassette: String,
    },
    
    /// Auto mode: replay if found, record if not.
    Auto {
        /// Port to listen on.
        #[arg(short, long, default_value = "8080")]
        port: u16,
        
        /// Cassette file.
        #[arg(short, long)]
        cassette: String,
        
        /// Target URL to proxy to.
        #[arg(short, long)]
        target: String,
    },
    
    /// List interactions in a cassette.
    List {
        /// Cassette file to list.
        cassette: String,
    },
    
    /// Show details of a specific interaction.
    Show {
        /// Cassette file.
        cassette: String,
        
        /// Interaction index (1-based).
        #[arg(short, long)]
        index: usize,
    },
    
    /// Remove interactions older than a specified age.
    Prune {
        /// Cassette file.
        cassette: String,
        
        /// Max age in days.
        #[arg(short, long, default_value = "30")]
        days: u32,
        
        /// Actually delete (without this flag, just shows what would be deleted).
        #[arg(long)]
        execute: bool,
    },
    
    /// Validate cassette files.
    Check {
        /// Cassette files to check.
        cassettes: Vec<String>,
    },
    
    /// Delete specific interactions by index.
    Delete {
        /// Cassette file.
        cassette: String,
        
        /// Interaction indices to delete (1-based, comma-separated).
        #[arg(short, long, value_delimiter = ',')]
        indices: Vec<usize>,
    },
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    
    if let Err(e) = run(args).await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

async fn run(args: Args) -> Result<()> {
    match args.command {
        Some(Command::Record { port, cassette: cassette_path, target }) => {
            let config = ProxyConfig::new(port, Mode::Record, &cassette_path)
                .target(&target);
            run_proxy(config).await?;
        }
        Some(Command::Replay { port, cassette: cassette_path }) => {
            let config = ProxyConfig::new(port, Mode::Replay, &cassette_path);
            run_proxy(config).await?;
        }
        Some(Command::Auto { port, cassette: cassette_path, target }) => {
            let config = ProxyConfig::new(port, Mode::Auto, &cassette_path)
                .target(&target);
            run_proxy(config).await?;
        }
        Some(Command::List { cassette: cassette_path }) => {
            let c = cassette::load_cassette(&cassette_path)?;
            println!("Cassette: {}", cassette_path);
            println!("Version: {}", c.version);
            println!("Interactions: {}", c.len());
            println!();
            
            for (i, interaction) in c.interactions.iter().enumerate() {
                println!(
                    "  {}. {} {} -> {}",
                    i + 1,
                    interaction.request.method,
                    interaction.request.url,
                    interaction.response.status
                );
            }
        }
        Some(Command::Show { cassette: cassette_path, index }) => {
            let c = cassette::load_cassette(&cassette_path)?;
            
            if index == 0 || index > c.len() {
                eprintln!("Invalid index {}. Cassette has {} interactions.", index, c.len());
                std::process::exit(1);
            }
            
            let interaction = &c.interactions[index - 1];
            
            println!("Request:");
            println!("  {} {}", interaction.request.method, interaction.request.url);
            for header in &interaction.request.headers {
                println!("  {}: {}", header.name, header.value);
            }
            if let Some(body) = &interaction.request.body {
                println!("  Body: {}", body);
            }
            
            println!();
            println!("Response:");
            println!("  Status: {}", interaction.response.status);
            for header in &interaction.response.headers {
                println!("  {}: {}", header.name, header.value);
            }
            if let Some(body) = &interaction.response.body {
                println!("  Body: {}", body);
            }
        }
        Some(Command::Prune { cassette: cassette_path, days, execute }) => {
            let mut c = cassette::load_cassette(&cassette_path)?;
            let cutoff = Utc::now() - Duration::days(days as i64);
            
            let old_len = c.len();
            let to_prune: Vec<_> = c.interactions.iter().enumerate()
                .filter(|(_, i)| i.recorded_at < cutoff)
                .map(|(idx, i)| (idx, i.request.method.clone(), i.request.url.clone(), i.recorded_at))
                .collect();
            
            if to_prune.is_empty() {
                println!("No interactions older than {} days.", days);
                return Ok(());
            }
            
            println!("Interactions older than {} days:", days);
            for (idx, method, url, recorded) in &to_prune {
                println!("  {}. {} {} (recorded {})", idx + 1, method, url, recorded.format("%Y-%m-%d"));
            }
            
            if execute {
                c.interactions.retain(|i| i.recorded_at >= cutoff);
                save_cassette(&cassette_path, &c)?;
                println!("\nPruned {} interactions. {} remaining.", old_len - c.len(), c.len());
            } else {
                println!("\nWould prune {} interactions. Use --execute to apply.", to_prune.len());
            }
        }
        Some(Command::Check { cassettes }) => {
            let mut errors = 0;
            
            for path in &cassettes {
                match cassette::load_cassette(path) {
                    Ok(c) => {
                        println!("{}: OK ({} interactions)", path, c.len());
                    }
                    Err(e) => {
                        println!("{}: ERROR - {}", path, e);
                        errors += 1;
                    }
                }
            }
            
            if errors > 0 {
                std::process::exit(1);
            }
        }
        Some(Command::Delete { cassette: cassette_path, indices }) => {
            let mut c = cassette::load_cassette(&cassette_path)?;
            
            // Validate indices
            for &idx in &indices {
                if idx == 0 || idx > c.len() {
                    eprintln!("Invalid index {}. Cassette has {} interactions.", idx, c.len());
                    std::process::exit(1);
                }
            }
            
            // Sort and reverse to delete from end first
            let mut sorted_indices = indices.to_vec();
            sorted_indices.sort_unstable();
            sorted_indices.reverse();
            sorted_indices.dedup();
            
            for idx in sorted_indices {
                let removed = c.interactions.remove(idx - 1);
                println!("Deleted: {} {}", removed.request.method, removed.request.url);
            }
            
            save_cassette(&cassette_path, &c)?;
            println!("\n{} interactions remaining.", c.len());
        }
        None => {
            println!("No command specified. Use --help for usage.");
        }
    }
    
    Ok(())
}
