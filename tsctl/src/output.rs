use crate::api::{Device, Key};
use comfy_table::{presets::UTF8_FULL, Table};

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

fn device_name(device: &Device) -> &str {
    if device.name.is_empty() {
        value_or_dash(&device.hostname)
    } else {
        &device.name
    }
}

pub fn print_device_list(devices: &[Device]) {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        "Name",
        "Node ID",
        "Address",
        "OS",
        "Version",
        "Auth",
        "Last Seen",
    ]);

    for device in devices {
        table.add_row(vec![
            device_name(device),
            value_or_dash(&device.node_id),
            device.addresses.first().map(String::as_str).unwrap_or("-"),
            value_or_dash(&device.os),
            value_or_dash(&device.client_version),
            yes_no(device.authorized),
            value_or_dash(&device.last_seen),
        ]);
    }

    println!("{table}");
}

pub fn print_client_list(clients: &[Key]) {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["Client ID", "Description", "Scopes", "Tags", "Created"]);

    for client in clients {
        table.add_row(vec![
            value_or_dash(&client.id).to_owned(),
            value_or_dash(&client.description).to_owned(),
            joined(&client.scopes),
            joined(&client.tags),
            value_or_dash(&client.created).to_owned(),
        ]);
    }

    println!("{table}");
}

pub fn print_device(device: &Device) {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);

    let rows = [
        ("Name", value_or_dash(&device.name).to_owned()),
        ("Hostname", value_or_dash(&device.hostname).to_owned()),
        ("Device ID", value_or_dash(&device.id).to_owned()),
        ("Node ID", value_or_dash(&device.node_id).to_owned()),
        ("User", value_or_dash(&device.user).to_owned()),
        ("Addresses", joined(&device.addresses)),
        ("OS", value_or_dash(&device.os).to_owned()),
        (
            "Client version",
            value_or_dash(&device.client_version).to_owned(),
        ),
        (
            "Update available",
            yes_no(device.update_available).to_owned(),
        ),
        ("Authorized", yes_no(device.authorized).to_owned()),
        ("External", yes_no(device.is_external).to_owned()),
        ("Created", value_or_dash(&device.created).to_owned()),
        ("Last seen", value_or_dash(&device.last_seen).to_owned()),
        ("Expires", value_or_dash(&device.expires).to_owned()),
        (
            "Key expiry disabled",
            yes_no(device.key_expiry_disabled).to_owned(),
        ),
        ("Tags", joined(&device.tags)),
        ("Advertised routes", joined(&device.advertised_routes)),
        ("Enabled routes", joined(&device.enabled_routes)),
        (
            "Blocks incoming",
            yes_no(device.blocks_incoming_connections).to_owned(),
        ),
        ("Machine key", value_or_dash(&device.machine_key).to_owned()),
        ("Node key", value_or_dash(&device.node_key).to_owned()),
        (
            "Tailnet Lock key",
            value_or_dash(&device.tailnet_lock_key).to_owned(),
        ),
        (
            "Tailnet Lock error",
            value_or_dash(&device.tailnet_lock_error).to_owned(),
        ),
    ];

    for (field, value) in rows {
        table.add_row(vec![field, &value]);
    }

    println!("{table}");
}
