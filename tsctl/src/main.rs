use api::{ApiError, TailscaleClient};
use clap::{ArgGroup, Parser, Subcommand, ValueEnum};
use serde_json::Value;
use std::io::{self, IsTerminal, Write};
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
    api_key: Option<String>,

    /// OAuth client ID (use with --client-secret)
    #[arg(long, env = "TS_CLIENT_ID")]
    client_id: Option<String>,

    /// OAuth client secret (use with --client-id)
    #[arg(long, env = "TS_CLIENT_SECRET", hide_env_values = true)]
    client_secret: Option<String>,

    /// Optional OAuth scopes to request; omit to use all scopes granted to the client
    #[arg(long, env = "TS_OAUTH_SCOPE", default_value = "")]
    oauth_scope: String,

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

        /// List OAuth client IDs instead of devices
        #[arg(short = 'c', long = "clients")]
        clients: bool,
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

        /// Device ID, node ID, MagicDNS name, or hostname
        #[arg(short = 'd', long)]
        device: Option<String>,

        /// Output format
        #[arg(short = 'o', long, value_enum)]
        output: Option<OutputFormat>,
    },

    /// Add a tag under ACL tagOwners
    Update {
        /// Tailnet ID; use '-' for the access token's tailnet
        #[arg(short = 'n', long, default_value = "-")]
        tailnet: String,

        /// Tag to add under tagOwners (for example tag:cluster-node)
        #[arg(short = 't', long = "tag", required = true)]
        tag: String,

        /// Skip the confirmation prompt
        #[arg(short = 'f', long = "force")]
        force: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Json,
    Yaml,
}

#[derive(Debug, Error)]
enum Error {
    #[error(
        "provide TS_API_KEY/--api-key, or both TS_CLIENT_ID/--client-id and TS_CLIENT_SECRET/--client-secret"
    )]
    MissingCredentials,
    #[error(transparent)]
    Api(#[from] ApiError),
    #[error("failed to encode JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("failed to encode YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("failed to read confirmation: {0}")]
    Io(#[from] io::Error),
    #[error("confirmation required; re-run with -f to skip the prompt")]
    ConfirmationRequired,
    #[error("update cancelled")]
    Cancelled,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();
    let client = build_client(&args).await?;
    match args.command {
        Command::List { tailnet, clients } => {
            if clients {
                let clients = client.list_oauth_clients(&tailnet).await?;
                if clients.is_empty() {
                    eprintln!(
                        "No OAuth clients visible. Use TS_CLIENT_ID/TS_CLIENT_SECRET to show the \
current client, or grant all:read (or a user API key) to enumerate all clients."
                    );
                }
                output::print_client_list(&clients);
            } else {
                let (devices, _) = client.list_devices(&tailnet, false).await?;
                output::print_device_list(&devices);
            }
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
            let (device, value) = client.resolve_device(&device).await?;
            match output {
                None => output::print_device(&device),
                Some(format) => print_structured(&value, format)?,
            }
        }
        Command::View { .. } => unreachable!("clap requires exactly one view target"),
        Command::Update {
            tailnet,
            tag,
            force,
        } => {
            let (policy, etag, update) = client.plan_acl_tag_owners(&tailnet, &[tag]).await?;
            if update.added.is_empty() {
                output::print_acl_tag_update(&tailnet, &update);
            } else {
                confirm_acl_update(
                    &tailnet,
                    &update.added,
                    client.oauth_client_id(),
                    force,
                )?;
                client.set_acl(&tailnet, &policy, etag.as_deref()).await?;
                output::print_acl_tag_update(&tailnet, &update);
            }
        }
    }

    Ok(())
}

async fn build_client(args: &Args) -> Result<TailscaleClient, Error> {
    let client_id = args
        .client_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let client_secret = args
        .client_secret
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    // Prefer OAuth client credentials when present. A leftover TS_API_KEY in the
    // environment otherwise silently wins and produces confusing 403s.
    match (client_id, client_secret) {
        (Some(client_id), Some(client_secret)) => Ok(TailscaleClient::from_oauth(
            &args.api_url,
            client_id,
            client_secret,
            &args.oauth_scope,
        )
        .await?),
        (None, None) => {
            if let Some(api_key) = args
                .api_key
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                Ok(TailscaleClient::new(&args.api_url, api_key.to_owned())?)
            } else {
                Err(Error::MissingCredentials)
            }
        }
        _ => Err(Error::MissingCredentials),
    }
}

fn print_structured(value: &Value, format: OutputFormat) -> Result<(), Error> {
    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(value)?),
        OutputFormat::Yaml => print!("{}", serde_yaml::to_string(value)?),
    }
    Ok(())
}

fn confirm_acl_update(
    tailnet: &str,
    tags: &[String],
    client_id: Option<&str>,
    force: bool,
) -> Result<(), Error> {
    if force {
        return Ok(());
    }
    if !io::stdin().is_terminal() {
        return Err(Error::ConfirmationRequired);
    }

    let tags = tags.join(", ");
    match client_id {
        Some(client_id) => eprint!(
            "Update ACL tagOwners for client {client_id} (tailnet {tailnet}): add {tags}? [y/N] "
        ),
        None => eprint!("Update ACL tagOwners for tailnet {tailnet}: add {tags}? [y/N] "),
    }
    io::stderr().flush()?;

    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    if is_confirmed(&answer) {
        Ok(())
    } else {
        Err(Error::Cancelled)
    }
}

fn is_confirmed(answer: &str) -> bool {
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
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

    #[test]
    fn accepts_oauth_client_credentials() {
        let args = Args::try_parse_from([
            "tsctl",
            "--client-id",
            "client",
            "--client-secret",
            "secret",
            "list",
            "-n",
            "-",
        ])
        .expect("oauth credentials should parse");
        assert_eq!(args.client_id.as_deref(), Some("client"));
        assert_eq!(args.client_secret.as_deref(), Some("secret"));
        assert!(args.oauth_scope.is_empty());
        assert!(args.api_key.is_none());
    }

    #[test]
    fn list_accepts_clients_flag() {
        let args = Args::try_parse_from(["tsctl", "--api-key", "secret", "list", "-c"])
            .expect("list -c should parse");
        assert!(matches!(
            args.command,
            Command::List {
                ref tailnet,
                clients: true,
            } if tailnet == "-"
        ));
    }

    #[test]
    fn list_clients_flag_keeps_tailnet() {
        let args = Args::try_parse_from([
            "tsctl",
            "--api-key",
            "secret",
            "list",
            "-n",
            "example.com",
            "--clients",
        ])
        .expect("list --clients should parse");
        assert!(matches!(
            args.command,
            Command::List {
                ref tailnet,
                clients: true,
            } if tailnet == "example.com"
        ));
    }

    #[test]
    fn update_requires_tag() {
        assert!(Args::try_parse_from(["tsctl", "--api-key", "secret", "update"]).is_err());
    }

    #[test]
    fn update_accepts_tag_input() {
        let args = Args::try_parse_from([
            "tsctl",
            "--api-key",
            "secret",
            "update",
            "-t",
            "tag:cluster-node",
        ])
        .expect("update -t should parse");
        assert!(matches!(
            args.command,
            Command::Update {
                ref tailnet,
                ref tag,
                force: false,
            } if tailnet == "-" && tag == "tag:cluster-node"
        ));
    }

    #[test]
    fn update_keeps_tailnet_and_tag() {
        let args = Args::try_parse_from([
            "tsctl",
            "--api-key",
            "secret",
            "update",
            "-n",
            "example.com",
            "--tag",
            "tag:vpn-gateway",
        ])
        .expect("update --tag should parse");
        assert!(matches!(
            args.command,
            Command::Update {
                ref tailnet,
                ref tag,
                force: false,
            } if tailnet == "example.com" && tag == "tag:vpn-gateway"
        ));
    }

    #[test]
    fn update_accepts_force_flag() {
        let args = Args::try_parse_from([
            "tsctl",
            "--api-key",
            "secret",
            "update",
            "-t",
            "tag:cluster-node",
            "-f",
        ])
        .expect("update -f should parse");
        assert!(matches!(
            args.command,
            Command::Update {
                ref tag,
                force: true,
                ..
            } if tag == "tag:cluster-node"
        ));
    }

    #[test]
    fn confirms_yes_answers() {
        assert!(is_confirmed("y"));
        assert!(is_confirmed("Y\n"));
        assert!(is_confirmed(" yes "));
        assert!(!is_confirmed(""));
        assert!(!is_confirmed("n"));
        assert!(!is_confirmed("no"));
    }
}
