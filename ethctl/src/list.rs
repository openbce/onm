use comfy_table::{presets::UTF8_FULL, Table};
use libonm::eth::{self, EthError};

pub async fn run() -> Result<(), EthError> {
    let interfaces = eth::list_interfaces()?;

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        "Name",
        "Addresses",
        "MTU",
        "State",
        "Kind",
        "Master",
        "Driver",
    ]);

    for iface in interfaces {
        let addresses = eth::get_interface_addresses(&iface.name)
            .await
            .unwrap_or_default();
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
            iface.driver.unwrap_or("-".to_string()),
        ]);
    }

    println!("{table}");

    Ok(())
}
