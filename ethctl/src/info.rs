use libonm::eth::{self, EthError};

pub fn run(name: &str) -> Result<(), EthError> {
    let iface = eth::get_interface(name)?;

    println!("Name:        {}", iface.name);
    println!("MAC Address: {}", iface.mac_address);
    println!("MTU:         {}", iface.mtu);
    println!("State:       {}", iface.state.to_string());
    println!(
        "Speed:       {}",
        iface
            .speed
            .map(|s| format!("{} Mbps", s))
            .unwrap_or("-".to_string())
    );
    println!(
        "Driver:      {}",
        iface.driver.unwrap_or("-".to_string())
    );
    println!(
        "PCI Slot:    {}",
        iface.pci_slot.unwrap_or("-".to_string())
    );

    Ok(())
}
