use clap::{Parser, Subcommand};

use tracing_subscriber::{filter::EnvFilter, filter::LevelFilter, fmt, prelude::*};

mod info;
mod list;
mod sysctl;

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
    /// Show detailed information of an interface and network sysctl settings
    Info {
        #[arg(short, long)]
        name: String,
    },
    /// Show network sysctl tuning parameters
    Sysctl,
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
        Commands::Info { name } => info::run(&name)?,
        Commands::Sysctl => sysctl::run(),
    }

    Ok(())
}
