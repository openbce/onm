use libonm::eth::{self, EthError};

pub fn run() -> Result<(), EthError> {
    let interfaces = eth::list_interfaces()?;

    println!(
        "{:<15} {:<20} {:<8} {:<10} {:<12} {:<15} {}",
        "Name", "MAC Address", "MTU", "State", "Speed(Mbps)", "Driver", "PCI Slot"
    );

    for iface in interfaces {
        println!(
            "{:<15} {:<20} {:<8} {:<10} {:<12} {:<15} {}",
            iface.name,
            iface.mac_address,
            iface.mtu,
            iface.state.to_string(),
            iface.speed.map(|s| s.to_string()).unwrap_or("-".to_string()),
            iface.driver.unwrap_or("-".to_string()),
            iface.pci_slot.unwrap_or("-".to_string()),
        );
    }

    Ok(())
}
