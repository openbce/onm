use crate::info::{generate_sysctl_output, print_sysctl_tables, OutputFormat, TuningProfile};

pub fn run(profile_str: &str, generate: Option<&str>) {
    let profile = TuningProfile::from_str(profile_str);

    match generate {
        Some(format_str) => {
            let format = OutputFormat::from_str(format_str);
            generate_sysctl_output(profile, format);
        }
        None => print_sysctl_tables(profile),
    }
}
