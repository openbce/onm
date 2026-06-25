use comfy_table::{presets::UTF8_FULL, Table};
use libonm::eth::{self, EthError};

pub fn run() -> Result<(), EthError> {
    let interfaces = eth::list_interfaces()?;

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        "Name",
        "MAC Address",
        "MTU",
        "State",
        "Speed(Mbps)",
        "Driver",
        "PCI Slot",
    ]);

    for iface in interfaces {
        table.add_row(vec![
            iface.name,
            iface.mac_address,
            iface.mtu.to_string(),
            iface.state.to_string(),
            iface
                .speed
                .map(|s| s.to_string())
                .unwrap_or("-".to_string()),
            iface.driver.unwrap_or("-".to_string()),
            iface.pci_slot.unwrap_or("-".to_string()),
        ]);
    }

    println!("{table}");

    Ok(())
}
