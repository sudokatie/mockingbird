//! mockingbird CLI.

use clap::Parser;
use mockingbird::cassette;
use mockingbird::error::Result;

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
        target: Option<String>,
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
    
    /// List interactions in a cassette.
    List {
        /// Cassette file to list.
        cassette: String,
    },
}

fn run(args: Args) -> Result<()> {
    match args.command {
        Some(Command::Record { port, cassette: cassette_path, target }) => {
            println!("Recording to {} on port {} (target: {:?})", cassette_path, port, target);
            println!("Not yet implemented");
        }
        Some(Command::Replay { port, cassette: cassette_path }) => {
            println!("Replaying from {} on port {}", cassette_path, port);
            println!("Not yet implemented");
        }
        Some(Command::List { cassette: cassette_path }) => {
            let c = cassette::load_cassette(&cassette_path)?;
            println!("Cassette: {}", cassette_path);
            println!("Version: {}", c.version);
            println!("Interactions: {}", c.len());
            
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
        None => {
            println!("No command specified. Use --help for usage.");
        }
    }
    
    Ok(())
}

fn main() {
    let args = Args::parse();
    
    if let Err(e) = run(args) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
