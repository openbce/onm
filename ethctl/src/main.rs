use clap::{Parser, Subcommand};

use tracing_subscriber::{filter::EnvFilter, filter::LevelFilter, fmt, prelude::*};

mod info;
mod link;
mod list;

#[derive(Parser)]
#[command(name = "ethctl")]
#[command(author = "Klaus Ma <klaus1982.cn@gmail.com>")]
#[command(version = "0.1.0")]
#[command(about = "Ethernet command line", long_about = None)]
struct Args {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List all ethernet interfaces
    List,
    /// Show network tuning info with all interfaces and suggested values
    Info {
        /// Tuning profile for suggested values: control-plane, worker (default: worker)
        #[arg(short, long, default_value = "worker")]
        profile: String,
    },
    /// Show ip link and ethtool settings with suggested values
    Link {
        #[arg(short, long)]
        name: String,
        /// Tuning profile for suggested values: control-plane, worker (default: worker)
        #[arg(short, long, default_value = "worker")]
        profile: String,
        /// Generate commands to apply suggested values: cmd, conf, script (default: cmd)
        #[arg(short, long, default_missing_value = "cmd", num_args = 0..=1)]
        generate: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

    tracing_subscriber::registry()
        .with(fmt::Layer::default().compact().with_writer(std::io::stderr))
        .with(env_filter)
        .try_init()?;

    let args = Args::parse();

    match args.command {
        Commands::List => list::run()?,
        Commands::Info { profile } => info::run(&profile)?,
        Commands::Link {
            name,
            profile,
            generate,
        } => link::run(&name, &profile, generate.as_deref()).await?,
    }

    Ok(())
}
