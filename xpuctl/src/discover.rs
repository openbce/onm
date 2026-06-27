use futures::future::join_all;
use libonm::xpu::{XPUError, XPU};

use crate::types::{Context, BMC};

struct DiscoverResult {
    name: String,
    address: String,
    status: String,
}

async fn discover_bmc(
    bmc: &BMC,
    username: &str,
    password: &str,
    tls_verify: bool,
) -> DiscoverResult {
    let status = match XPU::new(&bmc.to_libonm_bmc(username, password, tls_verify)).await {
        Ok(_) => "Ok".to_string(),
        Err(e) => e.to_string(),
    };
    DiscoverResult {
        name: bmc.name.clone(),
        address: bmc.address.clone(),
        status,
    }
}

pub async fn run(cxt: &Context) -> Result<(), XPUError> {
    println!("{:<20}{:<30}{:<50}", "Name", "BMC", "Status");

    let futures: Vec<_> = cxt
        .bmc
        .iter()
        .map(|bmc| discover_bmc(bmc, &cxt.username, &cxt.password, cxt.tls_verify))
        .collect();

    let results = join_all(futures).await;

    for result in results {
        println!(
            "{:<20}{:<30}{:<50}",
            result.name, result.address, result.status
        );
    }

    Ok(())
}
