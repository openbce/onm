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

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

mod types;
mod utils;
mod wrappers;

use std::alloc::{self, Layout};
use std::collections::HashMap;

use std::os::raw::c_int;
use std::ptr::NonNull;
use std::slice;
use std::{io, vec};

use numeric_cast::NumericCast;
use scopeguard::defer;

use wrappers::ib::{
    ibv_close_device, ibv_device_attr, ibv_free_device_list, ibv_get_device_list, ibv_gid,
    ibv_open_device, ibv_port_attr, ibv_query_device, ibv_query_gid, ibv_query_port,
};

use types::{DevicePtr, IbDevice, IbPort, IbPortLinkType, IbPortPhysState, IbPortState, PciDevice};
use utils::cstr_to_string;

/// List the HCAs on the host.
pub fn list_pci_devices() -> io::Result<Vec<PciDevice>> {
    let ib_ports = list_ib_ports()?;
    let context = libudev::Context::new()?;

    let mut enumerator = libudev::Enumerator::new(&context)?;
    enumerator.match_subsystem("infiniband")?;
    let devices = enumerator.scan_devices()?;

    let mut pci_devs = HashMap::<String, PciDevice>::new();
    for device in devices {
        if let Some(parent) = device.parent() {
            let pci_dev = PciDevice::try_from(parent)?;
            let pci_dev = pci_devs
                .entry(pci_dev.pci_slot_name.clone())
                .or_insert(pci_dev);

            let mut ib_dev = IbDevice::try_from(device)?;
            ib_dev.ib_ports = ib_ports
                .get(&ib_dev.name)
                .unwrap_or(&Vec::<IbPort>::new())
                .to_vec();

            pci_dev.fw_ver = ib_dev.fw_ver.clone();
            pci_dev.board_id = ib_dev.board_id.clone();

            pci_dev.ib_devices.push(ib_dev);
        }
    }

    Ok(pci_devs.into_values().collect())
}

fn list_ib_ports() -> io::Result<HashMap<String, Vec<IbPort>>> {
    let mut ib_ports = HashMap::<String, Vec<IbPort>>::new();

    unsafe {
        let mut num_devices: c_int = 0;
        let device_list = ibv_get_device_list(&mut num_devices);
        if device_list.is_null() {
            return Err(io::Error::last_os_error());
        }
        defer! {
            ibv_free_device_list(device_list);
        }

        let device_list: NonNull<DevicePtr> = NonNull::new_unchecked(device_list.cast());
        let len: usize = num_devices.numeric_cast();

        const MAX_DEVICES: usize = 256;
        if len > MAX_DEVICES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unreasonable device count: {}", len),
            ));
        }

        let devices = slice::from_raw_parts(device_list.as_ptr(), len);

        for devptr in devices {
            let ctx = ibv_open_device(devptr.ffi_ptr());
            if ctx.is_null() {
                return Err(io::Error::last_os_error());
            }
            defer! {
                ibv_close_device(ctx);
            };

            let dev_attr_ptr =
                alloc::alloc(Layout::new::<ibv_device_attr>()) as *mut ibv_device_attr;
            if dev_attr_ptr.is_null() {
                return Err(io::Error::new(
                    io::ErrorKind::OutOfMemory,
                    "allocation failed",
                ));
            }
            defer! {
                alloc::dealloc(dev_attr_ptr as *mut u8, Layout::new::<ibv_device_attr>());
            };

            if ibv_query_device(ctx, dev_attr_ptr) != 0 {
                return Err(io::Error::last_os_error());
            };

            let mut ports = vec![];

            for i in 1..=(*dev_attr_ptr).phys_port_cnt {
                let port_attr_ptr =
                    alloc::alloc(Layout::new::<ibv_port_attr>()) as *mut ibv_port_attr;
                if port_attr_ptr.is_null() {
                    return Err(io::Error::new(
                        io::ErrorKind::OutOfMemory,
                        "allocation failed",
                    ));
                }
                defer! {
                    alloc::dealloc(port_attr_ptr as *mut u8, Layout::new::<ibv_port_attr>());
                };

                if ibv_query_port(ctx, i, port_attr_ptr as *mut _) != 0 {
                    return Err(io::Error::last_os_error());
                };

                let gid_ptr = alloc::alloc(Layout::new::<ibv_gid>()) as *mut ibv_gid;
                if gid_ptr.is_null() {
                    return Err(io::Error::new(
                        io::ErrorKind::OutOfMemory,
                        "allocation failed",
                    ));
                }
                defer! {
                    alloc::dealloc(gid_ptr as *mut u8, Layout::new::<ibv_gid>());
                };

                if ibv_query_gid(ctx, i, 0, gid_ptr) != 0 {
                    return Err(io::Error::last_os_error());
                };

                let link_type = IbPortLinkType::try_from((*port_attr_ptr).link_layer)?;

                let (subnet, guid) = match link_type {
                    IbPortLinkType::Ethernet => (None, None),
                    IbPortLinkType::Infiniband => (
                        Some(format!(
                            "{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}",
                            (*gid_ptr).raw[0],
                            (*gid_ptr).raw[1],
                            (*gid_ptr).raw[2],
                            (*gid_ptr).raw[3],
                            (*gid_ptr).raw[4],
                            (*gid_ptr).raw[5],
                            (*gid_ptr).raw[6],
                            (*gid_ptr).raw[7]
                        )),
                        Some(format!(
                            "{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}",
                            (*gid_ptr).raw[8],
                            (*gid_ptr).raw[9],
                            (*gid_ptr).raw[10],
                            (*gid_ptr).raw[11],
                            (*gid_ptr).raw[12],
                            (*gid_ptr).raw[13],
                            (*gid_ptr).raw[14],
                            (*gid_ptr).raw[15]
                        )),
                    ),
                };

                ports.push(IbPort {
                    port_num: i,
                    lid: (*port_attr_ptr).lid,
                    link_type,
                    subnet,
                    guid,
                    state: IbPortState::try_from((*port_attr_ptr).state)?,
                    phys_state: IbPortPhysState::try_from((*port_attr_ptr).phys_state)?,
                });
            }

            ib_ports.insert(cstr_to_string((*devptr.ffi_ptr()).name.as_ptr()), ports);
        }
    };

    Ok(ib_ports)
}
