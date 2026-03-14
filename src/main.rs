//! mockingbird CLI.

use chrono::{Duration, Utc};
use clap::Parser;
use mockingbird::cassette::{self, save_cassette};
use mockingbird::error::Result;
use mockingbird::mode::Mode;
use mockingbird::{run_proxy, ProxyConfig};

/// Parse a duration string like "30d", "1w", "24h", "2m".
/// 
/// Supported units:
/// - d: days
/// - w: weeks  
/// - h: hours
/// - m: months (30 days)
fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim();
    if s.is_empty() {
        return Err(mockingbird::error::Error::Config("Empty duration string".into()));
    }
    
    // Find where the number ends and the unit begins
    let (num_str, unit) = s.split_at(
        s.find(|c: char| !c.is_ascii_digit())
            .unwrap_or(s.len())
    );
    
    let num: i64 = num_str.parse().map_err(|_| {
        mockingbird::error::Error::Config(format!("Invalid duration number: {}", num_str))
    })?;
    
    let unit = unit.trim().to_lowercase();
    let duration = match unit.as_str() {
        "d" | "day" | "days" => Duration::days(num),
        "w" | "week" | "weeks" => Duration::weeks(num),
        "h" | "hour" | "hours" => Duration::hours(num),
        "m" | "month" | "months" => Duration::days(num * 30), // Approximate
        "" => Duration::days(num), // Default to days if no unit
        _ => {
            return Err(mockingbird::error::Error::Config(
                format!("Unknown duration unit '{}'. Use d (days), w (weeks), h (hours), or m (months)", unit)
            ));
        }
    };
    
    Ok(duration)
}

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
    /// Start proxy server (unified command per spec).
    Serve {
        /// Port to listen on.
        #[arg(short, long, default_value = "8080")]
        port: u16,
        
        /// Cassette file path.
        #[arg(short, long)]
        cassette: String,
        
        /// Mode: record, playback/replay, auto.
        #[arg(short, long, default_value = "playback")]
        mode: String,
        
        /// Target URL to proxy to (required for record/auto modes).
        #[arg(short, long)]
        target: Option<String>,
        
        /// Host to bind to.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },
    
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
        
        /// Delete interactions older than this duration (e.g., 30d, 1w, 24h, 2m).
        /// Supported units: d (days), w (weeks), h (hours), m (months).
        #[arg(long, default_value = "30d")]
        older_than: String,
        
        /// Show what would be deleted without actually deleting.
        #[arg(long)]
        dry_run: bool,
    },
    
    /// Validate cassette files.
    Check {
        /// Cassette files to check (supports glob patterns like *.json).
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
    
    /// Re-record all interactions in a cassette by making real requests.
    Refresh {
        /// Cassette file to refresh.
        cassette: String,
        
        /// Target URL to send requests to.
        #[arg(short, long)]
        target: String,
        
        /// Create a backup before refreshing.
        #[arg(long, default_value = "true")]
        backup: bool,
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
        Some(Command::Serve { port, cassette: cassette_path, mode, target, host: _ }) => {
            let parsed_mode: Mode = mode.parse().map_err(|e: String| {
                mockingbird::error::Error::Config(e)
            })?;
            
            // Validate target is provided for modes that need it
            if parsed_mode.allows_real_requests() && target.is_none() {
                return Err(mockingbird::error::Error::Config(
                    "Target URL required for record/auto modes. Use --target <URL>".to_string()
                ));
            }
            
            let mut config = ProxyConfig::new(port, parsed_mode, &cassette_path);
            if let Some(t) = target {
                config = config.target(&t);
            }
            run_proxy(config).await?;
        }
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
                let status_str = if let Some(response) = &interaction.response {
                    format!("{}", response.status)
                } else if let Some(error) = &interaction.error {
                    format!("ERROR: {:?}", error.kind)
                } else {
                    "???".to_string()
                };
                println!(
                    "  {}. {} {} -> {}",
                    i + 1,
                    interaction.request.method,
                    interaction.request.url,
                    status_str
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
            if let Some(response) = &interaction.response {
                println!("Response:");
                println!("  Status: {}", response.status);
                for header in &response.headers {
                    println!("  {}: {}", header.name, header.value);
                }
                if let Some(body) = &response.body {
                    println!("  Body: {}", body);
                }
            } else if let Some(error) = &interaction.error {
                println!("Error:");
                println!("  Kind: {:?}", error.kind);
                println!("  Message: {}", error.message);
            } else {
                println!("(No response or error recorded)");
            }
        }
        Some(Command::Prune { cassette: cassette_path, older_than, dry_run }) => {
            let duration = parse_duration(&older_than)?;
            let mut c = cassette::load_cassette(&cassette_path)?;
            let cutoff = Utc::now() - duration;
            
            let old_len = c.len();
            let to_prune: Vec<_> = c.interactions.iter().enumerate()
                .filter(|(_, i)| i.recorded_at < cutoff)
                .map(|(idx, i)| (idx, i.request.method.clone(), i.request.url.clone(), i.recorded_at))
                .collect();
            
            if to_prune.is_empty() {
                println!("No interactions older than {}.", older_than);
                return Ok(());
            }
            
            println!("Interactions older than {}:", older_than);
            for (idx, method, url, recorded) in &to_prune {
                println!("  {}. {} {} (recorded {})", idx + 1, method, url, recorded.format("%Y-%m-%d"));
            }
            
            if dry_run {
                println!("\nDry run: would prune {} interactions.", to_prune.len());
            } else {
                c.interactions.retain(|i| i.recorded_at >= cutoff);
                save_cassette(&cassette_path, &c)?;
                println!("\nPruned {} interactions. {} remaining.", old_len - c.len(), c.len());
            }
        }
        Some(Command::Check { cassettes }) => {
            let mut errors = 0;
            let mut checked = 0;
            
            for pattern in &cassettes {
                // Expand glob patterns
                let paths: Vec<_> = match glob::glob(pattern) {
                    Ok(paths) => paths.filter_map(|r| r.ok()).collect(),
                    Err(_) => {
                        // Not a valid glob, treat as literal path
                        vec![std::path::PathBuf::from(pattern)]
                    }
                };
                
                if paths.is_empty() {
                    println!("{}: WARNING - no files matched", pattern);
                    continue;
                }
                
                for path in paths {
                    checked += 1;
                    let path_str = path.display().to_string();
                    match cassette::load_cassette(&path) {
                        Ok(c) => {
                            println!("{}: OK ({} interactions)", path_str, c.len());
                        }
                        Err(e) => {
                            println!("{}: ERROR - {}", path_str, e);
                            errors += 1;
                        }
                    }
                }
            }
            
            if checked == 0 {
                println!("No cassette files found.");
            } else if errors > 0 {
                println!("\n{}/{} cassettes had errors.", errors, checked);
                std::process::exit(1);
            } else {
                println!("\nAll {} cassettes OK.", checked);
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
        Some(Command::Refresh { cassette: cassette_path, target, backup }) => {
            let c = cassette::load_cassette(&cassette_path)?;
            
            if c.is_empty() {
                println!("Cassette is empty, nothing to refresh.");
                return Ok(());
            }
            
            // Create backup if requested
            if backup {
                let backup_path = format!("{}.bak", cassette_path);
                std::fs::copy(&cassette_path, &backup_path)?;
                println!("Backup created: {}", backup_path);
            }
            
            println!("Refreshing {} interactions...", c.len());
            
            let client = reqwest::Client::new();
            let mut new_cassette = mockingbird::cassette::Cassette::new();
            
            for (i, interaction) in c.interactions.iter().enumerate() {
                let req = &interaction.request;
                
                // Build full URL
                let full_url = if req.url.starts_with("http") {
                    req.url.clone()
                } else {
                    format!("{}{}", target.trim_end_matches('/'), req.url)
                };
                
                let method: reqwest::Method = req.method.parse()
                    .unwrap_or(reqwest::Method::GET);
                
                let mut builder = client.request(method, &full_url);
                
                // Add headers (skip host)
                for header in &req.headers {
                    if header.name.to_lowercase() != "host" {
                        builder = builder.header(&header.name, &header.value);
                    }
                }
                
                // Add body
                if let Some(body) = &req.body {
                    builder = builder.body(body.clone());
                }
                
                match builder.send().await {
                    Ok(response) => {
                        let status = response.status().as_u16();
                        let headers: Vec<mockingbird::cassette::Header> = response
                            .headers()
                            .iter()
                            .map(|(k, v)| mockingbird::cassette::Header::new(k.as_str(), v.to_str().unwrap_or("")))
                            .collect();
                        let body_bytes = response.bytes().await.ok();
                        
                        let mut recorded_response = mockingbird::cassette::RecordedResponse::new(status);
                        recorded_response.headers = headers;
                        if let Some(bytes) = body_bytes {
                            if !bytes.is_empty() {
                                if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                                    recorded_response.body = Some(text);
                                } else {
                                    use base64::Engine;
                                    recorded_response.body = Some(base64::engine::general_purpose::STANDARD.encode(&bytes));
                                    recorded_response.body_encoding = mockingbird::cassette::BodyEncoding::Base64;
                                }
                            }
                        }
                        
                        new_cassette.add(mockingbird::cassette::Interaction::new(
                            req.clone(),
                            recorded_response,
                        ));
                        println!("  {}. {} {} -> OK", i + 1, req.method, req.url);
                    }
                    Err(e) => {
                        eprintln!("  {}. {} {} -> ERROR: {}", i + 1, req.method, req.url, e);
                        // Keep the old interaction on error
                        new_cassette.add(interaction.clone());
                    }
                }
            }
            
            save_cassette(&cassette_path, &new_cassette)?;
            println!("\nRefreshed {} interactions.", new_cassette.len());
        }
        None => {
            println!("No command specified. Use --help for usage.");
        }
    }
    
    Ok(())
}
