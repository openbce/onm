use crate::api::Device;

fn value_or_dash(value: &str) -> &str {
    if value.is_empty() {
        "-"
    } else {
        value
    }
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn joined(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(", ")
    }
}

pub fn print_device_list(devices: &[Device]) {
    println!(
        "{:<28} {:<15} {:<10} {:<12} {:<8} LAST SEEN",
        "NAME", "ADDRESS", "OS", "VERSION", "AUTH"
    );
    for device in devices {
        let name = if device.name.is_empty() {
            value_or_dash(&device.hostname)
        } else {
            &device.name
        };
        println!(
            "{:<28} {:<15} {:<10} {:<12} {:<8} {}",
            name,
            device.addresses.first().map(String::as_str).unwrap_or("-"),
            value_or_dash(&device.os),
            value_or_dash(&device.client_version),
            yes_no(device.authorized),
            value_or_dash(&device.last_seen),
        );
    }
}

pub fn print_device(device: &Device) {
    println!("{:24}: {}", "Name", value_or_dash(&device.name));
    println!("{:24}: {}", "Hostname", value_or_dash(&device.hostname));
    println!("{:24}: {}", "Device ID", value_or_dash(&device.id));
    println!("{:24}: {}", "Node ID", value_or_dash(&device.node_id));
    println!("{:24}: {}", "User", value_or_dash(&device.user));
    println!("{:24}: {}", "Addresses", joined(&device.addresses));
    println!("{:24}: {}", "OS", value_or_dash(&device.os));
    println!(
        "{:24}: {}",
        "Client version",
        value_or_dash(&device.client_version)
    );
    println!(
        "{:24}: {}",
        "Update available",
        yes_no(device.update_available)
    );
    println!("{:24}: {}", "Authorized", yes_no(device.authorized));
    println!("{:24}: {}", "External", yes_no(device.is_external));
    println!("{:24}: {}", "Created", value_or_dash(&device.created));
    println!("{:24}: {}", "Last seen", value_or_dash(&device.last_seen));
    println!("{:24}: {}", "Expires", value_or_dash(&device.expires));
    println!(
        "{:24}: {}",
        "Key expiry disabled",
        yes_no(device.key_expiry_disabled)
    );
    println!("{:24}: {}", "Tags", joined(&device.tags));
    println!(
        "{:24}: {}",
        "Advertised routes",
        joined(&device.advertised_routes)
    );
    println!(
        "{:24}: {}",
        "Enabled routes",
        joined(&device.enabled_routes)
    );
    println!(
        "{:24}: {}",
        "Blocks incoming",
        yes_no(device.blocks_incoming_connections)
    );
    println!(
        "{:24}: {}",
        "Machine key",
        value_or_dash(&device.machine_key)
    );
    println!("{:24}: {}", "Node key", value_or_dash(&device.node_key));
    println!(
        "{:24}: {}",
        "Tailnet Lock key",
        value_or_dash(&device.tailnet_lock_key)
    );
    println!(
        "{:24}: {}",
        "Tailnet Lock error",
        value_or_dash(&device.tailnet_lock_error)
    );
}
