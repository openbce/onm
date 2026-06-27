use comfy_table::{presets::UTF8_FULL, Table};
use libonm::eth::{self, EthError};

pub async fn run(ipv4_only: bool, ipv6_only: bool) -> Result<(), EthError> {
    if ipv4_only && ipv6_only {
        return Err(EthError::InvalidConfig(
            "--ipv4 and --ipv6 cannot be used together".to_string(),
        ));
    }
    let routes = eth::get_routes().await?;

    let show_ipv4 = !ipv6_only;
    let show_ipv6 = !ipv4_only;

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        "Family",
        "Destination",
        "Gateway",
        "Interface",
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
