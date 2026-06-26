use comfy_table::{presets::UTF8_FULL, Table};
use libonm::eth::{self, EthError};

pub fn run() -> Result<(), EthError> {
    let nat_table = eth::get_nat_rules()?;

    if nat_table.rules.is_empty() {
        println!("No NAT rules found (SNAT/DNAT/MASQUERADE)");
        return Ok(());
    }

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        "Chain",
        "Type",
        "Protocol",
        "Source",
        "Destination",
        "In",
        "Out",
        "Target",
        "Packets",
        "Bytes",
    ]);

    for rule in &nat_table.rules {
        let target = match &rule.nat_type {
            libonm::eth::NatType::Snat => {
                rule.to_source.clone().map(|s| format!("to:{}", s)).unwrap_or("-".to_string())
            }
            libonm::eth::NatType::Dnat => {
                rule.to_destination.clone().map(|s| format!("to:{}", s)).unwrap_or("-".to_string())
            }
            libonm::eth::NatType::Masquerade => "-".to_string(),
        };

        let dest_with_port = match (&rule.destination, &rule.dport) {
            (Some(d), Some(p)) => format!("{}:{}", d, p),
            (Some(d), None) => d.clone(),
            (None, Some(p)) => format!("*:{}", p),
            (None, None) => "*".to_string(),
        };

        let src_with_port = match (&rule.source, &rule.sport) {
            (Some(s), Some(p)) => format!("{}:{}", s, p),
            (Some(s), None) => s.clone(),
            (None, Some(p)) => format!("*:{}", p),
            (None, None) => "*".to_string(),
        };

        table.add_row(vec![
            rule.chain.clone(),
            rule.nat_type.to_string(),
            rule.protocol.clone().unwrap_or("all".to_string()),
            src_with_port,
            dest_with_port,
            rule.interface_in.clone().unwrap_or("*".to_string()),
            rule.interface_out.clone().unwrap_or("*".to_string()),
            target,
            format_count(rule.packets),
            format_bytes(rule.bytes),
        ]);
    }

    println!("{table}");

    Ok(())
}

fn format_count(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1}G", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1}M", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1}K", bytes as f64 / 1024.0)
    } else {
        bytes.to_string()
    }
}
