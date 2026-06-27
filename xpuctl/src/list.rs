use futures::future::join_all;
use libonm::xpu::{XPUError, XPU};

use crate::types::{Context, BMC};

pub(crate) struct ListResult {
    name: String,
    status: String,
    vendor: String,
    firmware_version: String,
    serial_number: String,
    bmc_version: String,
    address: String,
}

pub(crate) async fn list_bmc(
    bmc: &BMC,
    username: &str,
    password: &str,
    tls_verify: bool,
) -> Result<ListResult, XPUError> {
    let xpu = XPU::new(&bmc.to_libonm_bmc(username, password, tls_verify)).await?;
    Ok(ListResult {
        name: bmc.name.clone(),
        status: xpu.status.to_string(),
        vendor: xpu.vendor,
        firmware_version: xpu.firmware_version,
        serial_number: xpu.serial_number,
        bmc_version: xpu.bmc_version,
        address: xpu.bmc.address,
    })
}

pub(crate) fn print_header() {
    println!(
        "{:<20}{:<10}{:<15}{:<10}{:<15}{:<15}{}",
        "ID", "Status", "Vendor", "FW", "SN", "BMC", "Address"
    );
}

pub(crate) fn print_result(result: &ListResult) {
    println!(
        "{:<20}{:<10}{:<15}{:<10}{:<15}{:<15}{}",
        result.name,
        result.status,
        result.vendor,
        result.firmware_version,
        result.serial_number,
        result.bmc_version,
        result.address,
    );
}

pub async fn run(cxt: &Context) -> Result<(), XPUError> {
    print_header();

    let futures: Vec<_> = cxt
        .bmc
        .iter()
        .map(|bmc| list_bmc(bmc, &cxt.username, &cxt.password, cxt.tls_verify))
        .collect();

    let results = join_all(futures).await;

    for result in results {
        let r = result?;
        print_result(&r);
    }

    Ok(())
}
