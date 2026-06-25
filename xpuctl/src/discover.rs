use libonm::xpu::{XPUError, BMC, XPU};

use crate::types::Context;

pub async fn run(cxt: &Context) -> Result<(), XPUError> {
    println!("{:<20}{:<30}{:<50}", "Name", "BMC", "Status");

    for bmc in cxt.bmc.iter() {
        match XPU::new(&BMC::from((bmc, cxt.username.as_str(), cxt.password.as_str()))).await {
            Ok(_) => println!("{:<20}{:<30}{:<50}", bmc.name, bmc.address, "Ok"),
            Err(e) => println!("{:<20}{:<30}{:<50}", bmc.name, bmc.address, e.to_string()),
        }
    }

    Ok(())
}
