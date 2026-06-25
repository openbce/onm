mod types;

use std::fs;
use std::path::Path;

pub use types::{EthError, EthInterface, LinkState};

const SYS_CLASS_NET: &str = "/sys/class/net";

pub fn list_interfaces() -> Result<Vec<EthInterface>, EthError> {
    let mut interfaces = Vec::new();

    let entries = fs::read_dir(SYS_CLASS_NET)?;

    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();

        if name == "lo" {
            continue;
        }

        let iface_path = entry.path();
        if !is_ethernet_device(&iface_path) {
            continue;
        }

        let iface = read_interface(&name, &iface_path)?;
        interfaces.push(iface);
    }

    interfaces.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(interfaces)
}

pub fn get_interface(name: &str) -> Result<EthInterface, EthError> {
    let iface_path = Path::new(SYS_CLASS_NET).join(name);
    if !iface_path.exists() {
        return Err(EthError::NotFound(name.to_string()));
    }

    read_interface(name, &iface_path)
}

fn is_ethernet_device(path: &Path) -> bool {
    let device_path = path.join("device");
    if !device_path.exists() {
        return false;
    }

    let type_path = path.join("type");
    if let Ok(type_content) = fs::read_to_string(&type_path) {
        if let Ok(dev_type) = type_content.trim().parse::<u32>() {
            return dev_type == 1;
        }
    }

    false
}

fn read_interface(name: &str, path: &Path) -> Result<EthInterface, EthError> {
    let mac_address = read_sysfs_file(path, "address").unwrap_or_default();
    let mtu = read_sysfs_file(path, "mtu")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1500);

    let state = match read_sysfs_file(path, "operstate").as_deref() {
        Some("up") => LinkState::Up,
        Some("down") => LinkState::Down,
        _ => LinkState::Unknown,
    };

    let speed = read_sysfs_file(path, "speed")
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|&s| s > 0 && s < 1000000);

    let driver = path
        .join("device/driver")
        .read_link()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));

    let pci_slot = path
        .join("device")
        .read_link()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));

    Ok(EthInterface {
        name: name.to_string(),
        mac_address,
        mtu,
        state,
        speed,
        driver,
        pci_slot,
    })
}

fn read_sysfs_file(base: &Path, file: &str) -> Option<String> {
    fs::read_to_string(base.join(file))
        .ok()
        .map(|s| s.trim().to_string())
}
