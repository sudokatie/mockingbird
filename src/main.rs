//! mockingbird CLI.

use clap::Parser;
use mockingbird::cassette;
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
        None => {
            println!("No command specified. Use --help for usage.");
        }
    }
    
    Ok(())
}
