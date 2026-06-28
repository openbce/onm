use comfy_table::{presets::UTF8_FULL, Table};
use libonm::eth::{self, EthError};

use crate::path::interface_paths;

pub async fn run() -> Result<(), EthError> {
    let interfaces = eth::list_interfaces()?;
    let paths = interface_paths(&interfaces);

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        "Name",
        "Addresses",
        "MTU",
        "State",
        "Kind",
        "Master",
        "Path",
        "Driver",
    ]);

    for iface in interfaces {
        let addresses = eth::get_interface_addresses(&iface.name).await?;
        let path = paths
            .get(&iface.name)
            .cloned()
            .unwrap_or_else(|| iface.name.clone());
        let addr_str = if addresses.is_empty() {
            "-".to_string()
        } else {
            addresses.join(", ")
        };

        table.add_row(vec![
            iface.name,
            addr_str,
            iface.mtu.to_string(),
            iface.state.to_string(),
            iface.kind.unwrap_or("-".to_string()),
            iface.master.unwrap_or("-".to_string()),
            path,
            iface.driver.unwrap_or("-".to_string()),
        ]);
    }

    println!("{table}");

    Ok(())
}
