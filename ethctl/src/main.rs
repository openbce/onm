use clap::{Parser, Subcommand};

use tracing_subscriber::{filter::EnvFilter, filter::LevelFilter, fmt, prelude::*};

mod format;
mod info;
mod link;
mod list;
mod nat;
mod path;
mod route;

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
        /// Tuning profile for suggested values: control-plane, worker, gateway (default: worker)
        #[arg(short, long, default_value = "worker")]
        profile: String,
        /// Output suggested values as commands: cmd, conf, script
        #[arg(short, long)]
        output: Option<String>,
        /// Generate backup of current sysctl values: cmd, conf
        #[arg(short, long)]
        backup: Option<String>,
    },
    /// Show ip link and ethtool settings with suggested values
    Link {
        #[arg(short, long)]
        name: String,
        /// Tuning profile for suggested values: control-plane, worker, gateway (default: worker)
        #[arg(short, long, default_value = "worker")]
        profile: String,
        /// Generate commands to apply suggested values: cmd, conf, script (default: cmd)
        #[arg(short, long, default_missing_value = "cmd", num_args = 0..=1)]
        generate: Option<String>,
    },
    /// Show routing table (IPv4 and IPv6)
    Route {
        /// Show only IPv4 routes
        #[arg(short = '4', long)]
        ipv4: bool,
        /// Show only IPv6 routes
        #[arg(short = '6', long)]
        ipv6: bool,
    },
    /// Show NAT rules (nftables and iptables)
    Nat {
        /// Filter by chain name (e.g., ts-postrouting, POSTROUTING)
        #[arg(short, long)]
        chain: Option<String>,
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
        Commands::List => list::run().await?,
        Commands::Info {
            profile,
            output,
            backup,
        } => info::run(&profile, output.as_deref(), backup.as_deref())?,
        Commands::Link {
            name,
            profile,
            generate,
        } => link::run(&name, &profile, generate.as_deref()).await?,
        Commands::Route { ipv4, ipv6 } => route::run(ipv4, ipv6).await?,
        Commands::Nat { chain } => nat::run(chain.as_deref())?,
    }

    Ok(())
}
