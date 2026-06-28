use number_prefix::NumberPrefix;

pub fn count(value: u64) -> String {
    match NumberPrefix::decimal(value as f64) {
        NumberPrefix::Standalone(_) => value.to_string(),
        NumberPrefix::Prefixed(prefix, scaled) => {
            format!(
                "{}{}",
                scaled.trunc() as u64,
                prefix.to_string().to_uppercase()
            )
        }
    }
}

pub fn binary(value: u64) -> String {
    match NumberPrefix::binary(value as f64) {
        NumberPrefix::Standalone(_) => value.to_string(),
        NumberPrefix::Prefixed(prefix, scaled) => {
            let suffix = prefix.to_string();
            format!("{}{suffix}", scaled.trunc() as u64)
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
            let suffix = prefix.to_string();
            format!("{} {suffix}B", scaled.trunc() as u64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_counts_and_bytes_with_compact_prefixes() {
        assert_eq!(count(999), "999");
        assert_eq!(count(1_000), "1K");
        assert_eq!(count(1_999), "1K");
        assert_eq!(count(1_000_000), "1M");
        assert_eq!(binary(1_572_864), "1Mi");
        assert_eq!(bytes(1_610_612_736), "1 GiB");
    }
}
