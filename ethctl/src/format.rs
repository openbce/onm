use number_prefix::NumberPrefix;

pub fn count(value: u64) -> String {
    match NumberPrefix::decimal(value as f64) {
        NumberPrefix::Standalone(_) => value.to_string(),
        NumberPrefix::Prefixed(prefix, scaled) => {
            format!("{scaled:.1}{}", prefix.to_string().to_uppercase())
        }
    }
}

pub fn binary(value: u64) -> String {
    match NumberPrefix::binary(value as f64) {
        NumberPrefix::Standalone(_) => value.to_string(),
        NumberPrefix::Prefixed(prefix, scaled) => {
            let suffix = prefix.to_string().replace('i', "").to_uppercase();
            format!("{scaled:.1}{suffix}")
        }
    }
}

pub fn bytes(value: u64) -> String {
    if value < 1024 {
        return format!("{value} B");
    }

    match NumberPrefix::binary(value as f64) {
        NumberPrefix::Standalone(_) => format!("{value} B"),
        NumberPrefix::Prefixed(prefix, scaled) => {
            let suffix = prefix.to_string().replace('i', "").to_uppercase();
            format!("{scaled:.1} {suffix}B")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_counts_and_bytes_with_compact_prefixes() {
        assert_eq!(count(999), "999");
        assert_eq!(count(1_000), "1.0K");
        assert_eq!(count(1_000_000), "1.0M");
        assert_eq!(binary(1_048_576), "1.0M");
        assert_eq!(bytes(1_073_741_824), "1.0 GB");
    }
}
