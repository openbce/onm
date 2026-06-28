use std::process::Command;

use super::{parse_iptables_nat, parse_nftables_json, same_nat_rule, EthError, NatRule, NatTable};

trait NatBackend {
    fn name(&self) -> &str;
    fn get_rules(&self) -> Result<Vec<NatRule>, EthError>;
}

struct NftablesBackend;

impl NatBackend for NftablesBackend {
    fn name(&self) -> &str {
        "nft"
    }

    fn get_rules(&self) -> Result<Vec<NatRule>, EthError> {
        let output = Command::new("nft")
            .args(["-j", "list", "ruleset"])
            .output()
            .map_err(|error| EthError::Internal(error.to_string()))?;

        if !output.status.success() {
            return Err(EthError::Internal(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }

        let mut table = NatTable::default();
        parse_nftables_json(&String::from_utf8_lossy(&output.stdout), &mut table)?;
        Ok(table.rules)
    }
}

struct IptablesBackend {
    command: &'static str,
    family: &'static str,
}

impl IptablesBackend {
    const fn new(command: &'static str, family: &'static str) -> Self {
        Self { command, family }
    }
}

impl NatBackend for IptablesBackend {
    fn name(&self) -> &str {
        self.command
    }

    fn get_rules(&self) -> Result<Vec<NatRule>, EthError> {
        let output = Command::new(self.command)
            .args(["-t", "nat", "-S"])
            .output()
            .map_err(|error| EthError::Internal(error.to_string()))?;

        if !output.status.success() {
            return Err(EthError::Internal(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }

        let mut table = NatTable::default();
        parse_iptables_nat(
            &String::from_utf8_lossy(&output.stdout),
            self.family,
            &mut table,
        );
        Ok(table.rules)
    }
}

/// Get NAT rules from all available nftables and iptables backends.
///
/// Every backend is queried because native nftables and iptables compatibility
/// rules can coexist. Duplicate rules reported through iptables-nft are merged.
pub fn get_nat_rules() -> Result<NatTable, EthError> {
    let nftables = NftablesBackend;
    let iptables = IptablesBackend::new("iptables", "ip");
    let ip6tables = IptablesBackend::new("ip6tables", "ip6");
    let backends: [&dyn NatBackend; 3] = [&nftables, &iptables, &ip6tables];

    collect_rules(&backends)
}

fn collect_rules(backends: &[&dyn NatBackend]) -> Result<NatTable, EthError> {
    let mut table = NatTable::default();
    let mut successful_backends = 0;
    let mut backend_errors = Vec::new();

    for backend in backends {
        match backend.get_rules() {
            Ok(rules) => {
                table.rules.extend(rules);
                successful_backends += 1;
            }
            Err(error) => backend_errors.push(format!("{}: {error}", backend.name())),
        }
    }

    if successful_backends == 0 {
        return Err(EthError::Internal(format!(
            "unable to query NAT rules: {}",
            backend_errors.join("; ")
        )));
    }

    deduplicate_rules(&mut table.rules);
    Ok(table)
}

fn deduplicate_rules(rules: &mut Vec<NatRule>) {
    let mut unique = Vec::with_capacity(rules.len());
    for rule in rules.drain(..) {
        if let Some(existing) = unique
            .iter_mut()
            .find(|existing| same_nat_rule(existing, &rule))
        {
            existing.packets = existing.packets.max(rule.packets);
            existing.bytes = existing.bytes.max(rule.bytes);
        } else {
            unique.push(rule);
        }
    }
    *rules = unique;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eth::NatType;

    struct FakeBackend {
        name: &'static str,
        result: Result<Vec<NatRule>, &'static str>,
    }

    impl NatBackend for FakeBackend {
        fn name(&self) -> &str {
            self.name
        }

        fn get_rules(&self) -> Result<Vec<NatRule>, EthError> {
            self.result
                .clone()
                .map_err(|error| EthError::Internal(error.to_string()))
        }
    }

    fn rule(packets: u64, bytes: u64) -> NatRule {
        NatRule {
            family: "ip".into(),
            chain: "POSTROUTING".into(),
            nat_type: NatType::Masquerade,
            source: None,
            destination: None,
            protocol: None,
            dport: None,
            sport: None,
            to_source: None,
            to_destination: None,
            interface_in: None,
            interface_out: None,
            packets,
            bytes,
        }
    }

    #[test]
    fn combines_successful_backends_and_deduplicates_rules() {
        let nft = FakeBackend {
            name: "nft",
            result: Ok(vec![rule(10, 100)]),
        };
        let iptables = FakeBackend {
            name: "iptables",
            result: Ok(vec![rule(0, 0)]),
        };
        let unavailable = FakeBackend {
            name: "ip6tables",
            result: Err("not found"),
        };

        let table = collect_rules(&[&nft, &iptables, &unavailable]).unwrap();
        assert_eq!(table.rules, vec![rule(10, 100)]);
    }

    #[test]
    fn fails_only_when_every_backend_fails() {
        let nft = FakeBackend {
            name: "nft",
            result: Err("not found"),
        };
        let iptables = FakeBackend {
            name: "iptables",
            result: Err("permission denied"),
        };

        let error = collect_rules(&[&nft, &iptables]).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("nft"));
        assert!(message.contains("iptables"));
    }
}
