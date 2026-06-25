use libonm::xpu::{XPUError, BMC, XPU};

use crate::types::Context;

pub async fn run(cxt: &Context) -> Result<(), XPUError> {
    println!(
        "{:<20}{:<10}{:<15}{:<10}{:<15}{:<15}{}",
        "ID", "Status", "Vendor", "FW", "SN", "BMC", "Address"
    );
    for bmc in cxt.bmc.iter() {
        let xpu = XPU::new(&BMC::from((bmc, cxt.username.as_str(), cxt.password.as_str()))).await?;
        println!(
            "{:<20}{:<10}{:<15}{:<10}{:<15}{:<15}{}",
            bmc.name,
            xpu.status.to_string(),
            xpu.vendor,
            xpu.firmware_version,
            xpu.serial_number,
            xpu.bmc_version,
            xpu.bmc.address,
        );
    }

    Ok(())
}
