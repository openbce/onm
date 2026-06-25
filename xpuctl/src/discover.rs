use libonm::xpu::{XPUError, XPU};

use crate::types::Context;

pub async fn run(cxt: &Context) -> Result<(), XPUError> {
    println!("{:<20}{:<30}{:<50}", "Name", "BMC", "Status");

    for bmc in cxt.bmc.iter() {
        match XPU::new(&bmc.to_libonm_bmc(&cxt.username, &cxt.password)).await {
            Ok(_) => println!("{:<20}{:<30}{:<50}", bmc.name, bmc.address, "Ok"),
            Err(e) => println!("{:<20}{:<30}{:<50}", bmc.name, bmc.address, e.to_string()),
        }
    }

    Ok(())
}
