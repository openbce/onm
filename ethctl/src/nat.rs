use comfy_table::{presets::UTF8_FULL, Table};
use libonm::eth::{self, EthError};
use std::collections::{HashMap, HashSet};

use crate::format;

pub fn run(chain_filter: Option<&str>) -> Result<(), EthError> {
    let nat_table = eth::get_nat_rules()?;

    let rules: Vec<_> = nat_table
        .rules
        .iter()
        .filter(|r| {
            chain_filter
                .map(|f| r.chain.to_lowercase().contains(&f.to_lowercase()))
                .unwrap_or(true)
        })
        .collect();

    if rules.is_empty() {
        if chain_filter.is_some() {
            println!("No NAT rules found matching chain filter");
        } else {
            println!("No NAT rules found (SNAT/DNAT/MASQUERADE)");
        }
        return Ok(());
    }

    let mut table = Table::new();
    let chain_graph = chain_graph(&nat_table.rules);
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        "Family",
        "Chain",
        "Path",
        "Type",
        "Protocol",
        "Source",
        "Destination",
        "In",
        "Out",
        "Target",
        "Packets",
        "Bytes",
    ]);

    for rule in &rules {
        let target = match &rule.nat_type {
            libonm::eth::NatType::Snat => rule
                .to_source
                .clone()
                .map(|s| format!("to:{}", s))
                .unwrap_or("-".to_string()),
            libonm::eth::NatType::Dnat => rule
                .to_destination
                .clone()
                .map(|s| format!("to:{}", s))
                .unwrap_or("-".to_string()),
            libonm::eth::NatType::Masquerade => "-".to_string(),
            libonm::eth::NatType::Jump(chain) => chain.clone(),
        };

        let dest_with_port = match (&rule.destination, &rule.dport) {
            (Some(d), Some(p)) => format!("{}:{}", d, p),
            (Some(d), None) => d.clone(),
            (None, Some(p)) => format!("*:{}", p),
            (None, None) => "*".to_string(),
        };

        let src_with_port = match (&rule.source, &rule.sport) {
            (Some(s), Some(p)) => format!("{}:{}", s, p),
            (Some(s), None) => s.clone(),
            (None, Some(p)) => format!("*:{}", p),
            (None, None) => "*".to_string(),
        };

        table.add_row(vec![
            rule.family.clone(),
            rule.chain.clone(),
            nat_paths(rule, &chain_graph),
            rule.nat_type.to_string(),
            rule.protocol.clone().unwrap_or("all".to_string()),
            src_with_port,
            dest_with_port,
            rule.interface_in.clone().unwrap_or("*".to_string()),
            rule.interface_out.clone().unwrap_or("*".to_string()),
            target,
            format::count(rule.packets),
            format::bytes(rule.bytes),
        ]);
    }

    println!("{table}");

    Ok(())
}

type ChainKey = (String, String);

fn chain_graph(rules: &[libonm::eth::NatRule]) -> HashMap<ChainKey, Vec<String>> {
    let mut incoming: HashMap<ChainKey, Vec<String>> = HashMap::new();
    for rule in rules {
        if let libonm::eth::NatType::Jump(target) = &rule.nat_type {
            let sources = incoming
                .entry((rule.family.clone(), target.clone()))
                .or_default();
            if !sources.contains(&rule.chain) {
                sources.push(rule.chain.clone());
            }
        }
    }
    incoming
}

fn paths_to_chain(
    family: &str,
    chain: &str,
    incoming: &HashMap<ChainKey, Vec<String>>,
    visiting: &mut HashSet<ChainKey>,
) -> Vec<Vec<String>> {
    let key = (family.to_string(), chain.to_string());
    if !visiting.insert(key.clone()) {
        return vec![vec![format!("{chain} [cycle]")]];
    }

    let mut paths = Vec::new();
    if let Some(sources) = incoming.get(&key) {
        for source in sources {
            for mut path in paths_to_chain(family, source, incoming, visiting) {
                path.push(chain.to_string());
                paths.push(path);
            }
        }
    } else {
        paths.push(vec![chain.to_string()]);
    }
    visiting.remove(&key);
    paths
}

fn nat_paths(rule: &libonm::eth::NatRule, incoming: &HashMap<ChainKey, Vec<String>>) -> String {
    let terminal = match &rule.nat_type {
        libonm::eth::NatType::Jump(target) => target.clone(),
        libonm::eth::NatType::Snat => rule
            .to_source
            .as_ref()
            .map(|target| format!("SNAT({target})"))
            .unwrap_or_else(|| "SNAT".to_string()),
        libonm::eth::NatType::Dnat => rule
            .to_destination
            .as_ref()
            .map(|target| format!("DNAT({target})"))
            .unwrap_or_else(|| "DNAT".to_string()),
        libonm::eth::NatType::Masquerade => "MASQUERADE".to_string(),
    };

    let mut paths: Vec<String> =
        paths_to_chain(&rule.family, &rule.chain, incoming, &mut HashSet::new())
            .into_iter()
            .map(|mut path| {
                path.push(terminal.clone());
                path.join(" → ")
            })
            .collect();
    paths.sort();
    paths.dedup();
    paths.join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use libonm::eth::{NatRule, NatType};

    fn rule(chain: &str, nat_type: NatType) -> NatRule {
        NatRule {
            family: "ip".into(),
            chain: chain.into(),
            nat_type,
            source: None,
            destination: None,
            protocol: None,
            dport: None,
            sport: None,
            to_source: None,
            to_destination: None,
            interface_in: None,
            interface_out: None,
            packets: 0,
            bytes: 0,
        }
    }

    #[test]
    fn expands_chain_jumps_from_root_to_terminal() {
        let rules = vec![
            rule("POSTROUTING", NatType::Jump("vendor-nat".into())),
            rule("vendor-nat", NatType::Jump("workload-nat".into())),
            rule("workload-nat", NatType::Masquerade),
        ];
        let graph = chain_graph(&rules);

        assert_eq!(
            nat_paths(&rules[2], &graph),
            "POSTROUTING → vendor-nat → workload-nat → MASQUERADE"
        );
    }
}
