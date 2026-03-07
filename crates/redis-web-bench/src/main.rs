use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Compare a base redis-web config against named variants.
    Compare {
        /// Path to a YAML or JSON benchmark spec.
        #[arg(long)]
        spec: PathBuf,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Compare { spec } => {
            let artifact_dir = redis_web_bench::run_compare(&spec).await?;
            println!("Wrote benchmark artifacts to {}", artifact_dir.display());
        }
    }

    Ok(())
}
