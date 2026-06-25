use clap::{Parser, Subcommand};
use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::PathBuf;

use tracing_subscriber::{filter::EnvFilter, filter::LevelFilter, fmt as tracing_fmt, prelude::*};

use types::Context;

mod discover;
mod list;
mod types;
mod view;

#[derive(Debug)]
enum ConfigError {
    FileNotFound(PathBuf),
    ReadError(PathBuf, std::io::Error),
    ParseError(PathBuf, toml::de::Error),
    InvalidPath(PathBuf),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::FileNotFound(path) => write!(f, "configuration file not found: {}", path.display()),
            ConfigError::ReadError(path, e) => write!(f, "failed to read {}: {}", path.display(), e),
            ConfigError::ParseError(path, e) => write!(f, "failed to parse {}: {}", path.display(), e),
            ConfigError::InvalidPath(path) => write!(f, "invalid path: {}", path.display()),
        }
    }
}

impl Error for ConfigError {}

#[derive(Parser, Debug)]
#[command(name = "xpuctl")]
#[command(author = "Klaus Ma <klaus1982.cn@gmail.com>")]
#[command(version = "0.1.0")]
#[command(about = "XPU command line", long_about = None)]
struct Args {
    #[clap(flatten)]
    options: Options,

    #[clap(subcommand)]
    subcommand: SubCommand,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Options {
    #[arg(long = "config-file", short = 'c', default_value_t=("~/.xpuctl").to_string())]
    config_file: String,
}

#[derive(Debug, Subcommand, Clone)]
enum SubCommand {
    List,
    View {
        #[arg(long = "xpu", short = 'x')]
        xpu: usize,
    },
    Discover,
}

fn expand_path(path_str: &str) -> Result<PathBuf, ConfigError> {
    use std::path::Path;
    let path = Path::new(path_str);
    let mut expanded = PathBuf::new();

    for component in path.iter() {
        if component == "~" {
            let home = env::var("HOME")
                .map_err(|_| ConfigError::InvalidPath(path.to_path_buf()))?;
            expanded.push(home);
        } else {
            expanded.push(component);
        }
    }

    Ok(expanded)
}

fn load_config(path_str: &str) -> Result<Context, ConfigError> {
    let config_file = expand_path(path_str)?;

    if !config_file.exists() {
        return Err(ConfigError::FileNotFound(config_file));
    }

    let contents = fs::read_to_string(&config_file)
        .map_err(|e| ConfigError::ReadError(config_file.clone(), e))?;

    let config: Context = toml::from_str(&contents)
        .map_err(|e| ConfigError::ParseError(config_file, e))?;

    Ok(config)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy()
        .add_directive("tower=warn".parse()?)
        .add_directive("rustls=warn".parse()?)
        .add_directive("reqwest=info".parse()?)
        .add_directive("hyper=info".parse()?)
        .add_directive("h2=warn".parse()?);

    tracing_subscriber::registry()
        .with(tracing_fmt::Layer::default().compact().with_writer(std::io::stderr))
        .with(env_filter)
        .try_init()?;

    let args = Args::parse();

    let mut cxt = load_config(&args.options.config_file)?;

    for bmc in cxt.bmc.iter_mut() {
        if bmc.password.is_none() {
            bmc.password = Some(cxt.password.clone());
        }

        if bmc.username.is_none() {
            bmc.username = Some(cxt.username.clone());
        }
    }

    match &args.subcommand {
        SubCommand::Discover => discover::run(&cxt).await?,
        SubCommand::List => list::run(&cxt).await?,
        SubCommand::View { xpu } => view::run(&cxt, *xpu).await?,
    }

    Ok(())
}
