use std::collections::{HashMap, HashSet};

use libonm::eth::EthInterface;

pub fn interface_paths(interfaces: &[EthInterface]) -> HashMap<String, String> {
    let masters: HashMap<&str, &str> = interfaces
        .iter()
        .filter_map(|iface| {
            iface
                .master
                .as_deref()
                .map(|master| (iface.name.as_str(), master))
        })
        .collect();

    interfaces
        .iter()
        .map(|iface| {
            let mut components = vec![iface.name.clone()];
            let mut current = iface.name.as_str();
            let mut visited = HashSet::from([current]);

            while let Some(master) = masters.get(current).copied() {
                components.push(master.to_string());
                if !visited.insert(master) {
                    components.push("[cycle]".to_string());
                    break;
                }
                current = master;
            }

            (iface.name.clone(), components.join(" → "))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_nested_interface_masters() {
        let interfaces = vec![
            EthInterface {
                name: "eth0".into(),
                master: Some("bond0".into()),
                ..Default::default()
            },
            EthInterface {
                name: "bond0".into(),
                master: Some("br0".into()),
                ..Default::default()
            },
            EthInterface {
                name: "br0".into(),
                ..Default::default()
            },
        ];

        assert_eq!(
            interface_paths(&interfaces).get("eth0").unwrap(),
            "eth0 → bond0 → br0"
        );
    }
}
