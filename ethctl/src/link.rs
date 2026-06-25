use crate::info::{generate_link_output, print_link_tables, OutputFormat, TuningProfile};
use libonm::eth::EthError;

pub async fn run(name: &str, profile_str: &str, generate: Option<&str>) -> Result<(), EthError> {
    let profile = TuningProfile::from_str(profile_str);

    match generate {
        Some(format_str) => {
            let format = OutputFormat::from_str(format_str);
            generate_link_output(name, profile, format).await;
        }
        None => print_link_tables(name, profile).await,
    }

    Ok(())
}
