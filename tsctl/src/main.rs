use api::{ApiError, TailscaleClient};
use clap::{ArgGroup, Parser, Subcommand, ValueEnum};
use serde_json::Value;
use thiserror::Error;

mod api;
mod output;

const DEFAULT_API_URL: &str = "https://api.tailscale.com/api/v2/";

#[derive(Debug, Parser)]
#[command(
    name = "tsctl",
    author = "Klaus Ma <klaus1982.cn@gmail.com>",
    version,
    about = "Tailscale REST API command line"
)]
struct Args {
    /// Tailscale API access token
    #[arg(long, env = "TS_API_KEY", hide_env_values = true)]
    api_key: String,

    /// Tailscale API v2 base URL
    #[arg(long, env = "TS_API_URL", default_value = DEFAULT_API_URL)]
    api_url: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List devices in a tailnet
    List {
        /// Tailnet ID; use '-' for the access token's tailnet
        #[arg(short = 'n', long, default_value = "-")]
        tailnet: String,
    },

    /// View a tailnet or device
    #[command(group(
        ArgGroup::new("target")
            .required(true)
            .multiple(false)
            .args(["tailnet", "device"])
    ))]
    View {
        /// Tailnet ID; use '-' for the access token's tailnet
        #[arg(short = 'n', long)]
        tailnet: Option<String>,

        /// Device ID or node ID
        #[arg(short = 'd', long)]
        device: Option<String>,

        /// Output format
        #[arg(short = 'o', long, value_enum)]
        output: Option<OutputFormat>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Json,
    Yaml,
}

#[derive(Debug, Error)]
enum Error {
    #[error("TS_API_KEY or --api-key is required")]
    MissingApiKey,
    #[error(transparent)]
    Api(#[from] ApiError),
    #[error("failed to encode JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("failed to encode YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();
    if args.api_key.trim().is_empty() {
        return Err(Error::MissingApiKey);
    }

    let client = TailscaleClient::new(&args.api_url, args.api_key)?;
    match args.command {
        Command::List { tailnet } => {
            let (devices, _) = client.list_devices(&tailnet, false).await?;
            output::print_device_list(&devices);
        }
        Command::View {
            tailnet: Some(tailnet),
            device: None,
            output,
        } => {
            let (devices, value) = client.list_devices(&tailnet, true).await?;
            match output {
                None => {
                    println!("Tailnet: {tailnet}");
                    println!("Devices: {}\n", devices.len());
                    output::print_device_list(&devices);
                }
                Some(format) => print_structured(&value, format)?,
            }
        }
        Command::View {
            tailnet: None,
            device: Some(device),
            output,
        } => {
            let (device, value) = client.get_device(&device).await?;
            match output {
                None => output::print_device(&device),
                Some(format) => print_structured(&value, format)?,
            }
        }
        Command::View { .. } => unreachable!("clap requires exactly one view target"),
    }

    Ok(())
}

fn print_structured(value: &Value, format: OutputFormat) -> Result<(), Error> {
    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(value)?),
        OutputFormat::Yaml => print!("{}", serde_yaml::to_string(value)?),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn view_accepts_tailnet() {
        let args = Args::try_parse_from(["tsctl", "--api-key", "secret", "view", "-n", "-"])
            .expect("tailnet target should parse");
        assert!(matches!(
            args.command,
            Command::View {
                tailnet: Some(ref value),
                device: None,
                output: None,
            } if value == "-"
        ));
    }

    #[test]
    fn view_accepts_device() {
        let args = Args::try_parse_from([
            "tsctl",
            "--api-key",
            "secret",
            "view",
            "-d",
            "node-id",
            "--output",
            "yaml",
        ])
        .expect("device target should parse");
        assert!(matches!(
            args.command,
            Command::View {
                tailnet: None,
                device: Some(ref value),
                output: Some(OutputFormat::Yaml),
            } if value == "node-id"
        ));
    }

    #[test]
    fn view_rejects_ambiguous_target() {
        assert!(Args::try_parse_from([
            "tsctl",
            "--api-key",
            "secret",
            "view",
            "-n",
            "-",
            "-d",
            "node-id"
        ])
        .is_err());
    }
}
