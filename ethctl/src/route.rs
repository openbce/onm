use comfy_table::{presets::UTF8_FULL, Table};
use libonm::eth::{self, EthError};

use crate::path::interface_paths;

pub async fn run(ipv4_only: bool, ipv6_only: bool) -> Result<(), EthError> {
    if ipv4_only && ipv6_only {
        return Err(EthError::InvalidConfig(
            "--ipv4 and --ipv6 cannot be used together".to_string(),
        ));
    }
    let routes = eth::get_routes().await?;
    let interfaces = eth::list_interfaces()?;
    let paths = interface_paths(&interfaces);

    let show_ipv4 = !ipv6_only;
    let show_ipv6 = !ipv4_only;

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        "Family",
        "Destination",
        "Gateway",
        "Interface",
        "Path",
        "Metric",
        "Protocol",
        "Scope",
        "Type",
    ]);

    if show_ipv4 {
        for route in &routes.ipv4 {
            table.add_row(vec![
                "IPv4".to_string(),
                route.destination.clone(),
                route.gateway.clone().unwrap_or("-".to_string()),
                route.interface.clone().unwrap_or("-".to_string()),
                route_path(route, &paths),
                route
                    .metric
                    .map(|m| m.to_string())
                    .unwrap_or("-".to_string()),
                route.protocol.to_string(),
                route.scope.to_string(),
                route.route_type.to_string(),
            ]);
        }
    }

    if show_ipv6 {
        for route in &routes.ipv6 {
            table.add_row(vec![
                "IPv6".to_string(),
                route.destination.clone(),
                route.gateway.clone().unwrap_or("-".to_string()),
                route.interface.clone().unwrap_or("-".to_string()),
                route_path(route, &paths),
                route
                    .metric
                    .map(|m| m.to_string())
                    .unwrap_or("-".to_string()),
                route.protocol.to_string(),
                route.scope.to_string(),
                route.route_type.to_string(),
            ]);
        }
    }

    if routes.ipv4.is_empty() && routes.ipv6.is_empty() {
        println!("No routes found");
    } else {
        println!("{table}");
    }

    Ok(())
}

fn route_path(
    route: &libonm::eth::RouteEntry,
    interface_paths: &std::collections::HashMap<String, String>,
) -> String {
    let mut components = vec![route.destination.clone()];
    if let Some(gateway) = &route.gateway {
        components.push(format!("via {gateway}"));
    }
    if let Some(interface) = &route.interface {
        components.push(
            interface_paths
                .get(interface)
                .cloned()
                .unwrap_or_else(|| interface.clone()),
        );
    }
    components.join(" → ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combines_route_and_interface_hierarchy() {
        let route = libonm::eth::RouteEntry {
            destination: "10.0.0.0/24".into(),
            gateway: Some("192.0.2.1".into()),
            interface: Some("eth0".into()),
            ..Default::default()
        };
        let paths = std::collections::HashMap::from([(
            "eth0".to_string(),
            "eth0 → bond0 → br0".to_string(),
        )]);

        assert_eq!(
            route_path(&route, &paths),
            "10.0.0.0/24 → via 192.0.2.1 → eth0 → bond0 → br0"
        );
    }
}
