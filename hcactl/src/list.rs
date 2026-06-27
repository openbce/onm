/*
Copyright 2023 The xflops Authors.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

use libonm::hca;

pub fn run() -> Result<(), color_eyre::Report> {
    let hcas = hca::list_pci_devices()?;

    for hca in hcas {
        println!("----------------------------------------------");

        println!("{:<15}: {}", "ID", hca.pci_slot_name);
        println!("{:<15}: {}", "Subsystem ID", hca.subsys_id);
        println!("{:<15}: {}", "Model", hca.model_name);
        println!("{:<15}: {}", "Vendor", hca.vendor_name);
        println!("{:<15}: {}", "FW", hca.fw_ver);
        println!("{:<15}: {}", "Board", hca.board_id);

        println!();

        println!(
            "    {:<15}{:<15}{:<25}{:<25}{:<15}{:<25}{:<15}{:<15}{:<15}",
            "Name",
            "Slot",
            "Node GUID",
            "Port GUID",
            "LID",
            "Subnet",
            "LinkType",
            "State",
            "PhysState"
        );

        for dev in hca.ib_devices {
            for port in dev.ib_ports {
                println!(
                    "    {:<15}{:<15}{:<25}{:<25}{:<15}{:<25}{:<15}{:<15}{:<15}",
                    dev.name,
                    dev.slot_name,
                    dev.node_guid,
                    port.guid.unwrap_or("-".to_string()),
                    port.lid,
                    port.subnet.unwrap_or("-".to_string()),
                    port.link_type.to_string(),
                    port.state.to_string(),
                    port.phys_state.to_string(),
                );
            }
        }

        println!();
        println!();
    }

    Ok(())
}
