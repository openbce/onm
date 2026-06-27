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

use std::fmt::{self, Display};
use std::io;
use std::ptr::NonNull;

use libudev::Device;

use super::utils::{get_property, get_sysattr};
use super::wrappers::ib::{self, ibv_device, ibv_device_attr};

#[derive(Clone)]
pub struct PciDevice {
    /// The domain:bus:device.function address, which uniquely identifies this
    /// PCI function on the current host.
    pub pci_slot_name: String,
    /// The subsystem vendor/device identifier. Multiple physical devices of
    /// the same model can (and usually do) share this value.
    pub subsys_id: String,
    pub model_name: String,
    pub vendor_name: String,
    pub vendor: String,
    pub board_id: String,
    pub fw_ver: String,
    pub ib_devices: Vec<IbDevice>,
}

impl TryFrom<Device> for PciDevice {
    type Error = io::Error;
    fn try_from(dev: Device) -> Result<Self, Self::Error> {
        Ok(Self {
            pci_slot_name: dev
                .property_value("PCI_SLOT_NAME")
                .and_then(|value| value.to_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .or_else(|| {
                    dev.sysname()
                        .map(|value| value.to_string_lossy().into_owned())
                })
                .or_else(|| {
                    dev.syspath()
                        .map(|value| value.to_string_lossy().into_owned())
                })
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "PCI device has no stable identity",
                    )
                })?,
            subsys_id: get_property(&dev, "PCI_SUBSYS_ID")?.to_string(),
            model_name: get_property(&dev, "ID_MODEL_FROM_DATABASE")?.to_string(),
            vendor_name: get_property(&dev, "ID_VENDOR_FROM_DATABASE")?.to_string(),
            vendor: get_sysattr(&dev, "vendor")?.to_string(),
            ib_devices: vec![],

            board_id: String::new(),
            fw_ver: String::new(),
        })
    }
}

#[derive(Clone)]
pub struct IbDevice {
    pub name: String,
    pub slot_name: String,
    pub node_guid: String,
    pub node_desc: String,
    pub sys_image_guid: String,
    pub fw_ver: String,
    pub board_id: String,
    pub ib_ports: Vec<IbPort>,
}

impl TryFrom<Device> for IbDevice {
    type Error = io::Error;
    fn try_from(dev: Device) -> Result<Self, Self::Error> {
        let slot_name = match dev.parent() {
            Some(p) => get_property(&p, "PCI_SLOT_NAME")?.to_string(),
            None => String::new(),
        };
        Ok(Self {
            name: get_property(&dev, "NAME")?.to_string(),
            slot_name,
            node_guid: get_sysattr(&dev, "node_guid")?.to_string(),
            node_desc: get_sysattr(&dev, "node_desc")?.to_string(),
            sys_image_guid: get_sysattr(&dev, "sys_image_guid")?.to_string(),
            fw_ver: get_sysattr(&dev, "fw_ver")?.to_string(),
            board_id: get_sysattr(&dev, "board_id")?.to_string(),
            ib_ports: vec![],
        })
    }
}

#[derive(Clone)]
pub enum IbPortLinkType {
    Ethernet,
    Infiniband,
}

impl TryFrom<u8> for IbPortLinkType {
    type Error = io::Error;
    fn try_from(v: u8) -> io::Result<Self> {
        match v {
            1 => Ok(Self::Infiniband),
            2 => Ok(Self::Ethernet),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid port link type: {}", v),
            )),
        }
    }
}

impl Display for IbPortLinkType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ethernet => f.write_str("Eth"),
            Self::Infiniband => f.write_str("IB"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IbPortState {
    Nop,
    Down,
    Initializing,
    Armed,
    Active,
    ActiveDefer,
    Unknown(u32),
}

impl Display for IbPortState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nop => write!(f, "Nop"),
            Self::Down => write!(f, "Down"),
            Self::Initializing => write!(f, "Initializing"),
            Self::Armed => write!(f, "Armed"),
            Self::Active => write!(f, "Active"),
            Self::ActiveDefer => write!(f, "ActiveDefer"),
            Self::Unknown(value) => write!(f, "Unknown({value})"),
        }
    }
}

impl TryFrom<u32> for IbPortState {
    type Error = io::Error;
    fn try_from(v: u32) -> io::Result<Self> {
        match v {
            0 => Ok(Self::Nop),
            ib::ibv_port_state::IBV_PORT_DOWN => Ok(Self::Down),
            ib::ibv_port_state::IBV_PORT_INIT => Ok(Self::Initializing),
            3 => Ok(Self::Armed),
            ib::ibv_port_state::IBV_PORT_ACTIVE => Ok(Self::Active),
            5 => Ok(Self::ActiveDefer),
            _ => Ok(Self::Unknown(v)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IbPortPhysState {
    Nop,
    Sleep,
    Polling,
    Disabled,
    PortConfigurationTraining,
    LinkUp,
    LinkErrorRecovery,
    PhyTest,
    Unknown(u8),
}

impl Display for IbPortPhysState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nop => f.write_str("Nop"),
            Self::Sleep => f.write_str("Sleep"),
            Self::Polling => f.write_str("Polling"),
            Self::Disabled => f.write_str("Disabled"),
            Self::PortConfigurationTraining => f.write_str("PortConfigurationTraining"),
            Self::LinkUp => f.write_str("LinkUp"),
            Self::LinkErrorRecovery => f.write_str("LinkErrorRecovery"),
            Self::PhyTest => f.write_str("PhyTest"),
            Self::Unknown(value) => write!(f, "Unknown({value})"),
        }
    }
}

impl TryFrom<u8> for IbPortPhysState {
    type Error = io::Error;
    fn try_from(v: u8) -> io::Result<Self> {
        match v {
            0 => Ok(Self::Nop),
            1 => Ok(Self::Sleep),
            2 => Ok(Self::Polling),
            3 => Ok(Self::Disabled),
            4 => Ok(Self::PortConfigurationTraining),
            5 => Ok(Self::LinkUp),
            6 => Ok(Self::LinkErrorRecovery),
            7 => Ok(Self::PhyTest),
            _ => Ok(Self::Unknown(v)),
        }
    }
}

#[derive(Clone)]
pub struct IbPort {
    pub port_num: u8,
    pub guid: Option<String>,
    pub subnet: Option<String>,
    pub lid: u16,
    pub link_type: IbPortLinkType,
    pub state: IbPortState,
    pub phys_state: IbPortPhysState,
}

#[allow(missing_copy_implementations)] // This type can not copy
#[repr(transparent)]
pub struct DevicePtr(NonNull<ibv_device>);

impl DevicePtr {
    pub fn ffi_ptr(&self) -> *mut ibv_device {
        self.0.as_ptr()
    }
}

#[allow(missing_copy_implementations)] // This type can not copy
#[repr(transparent)]
pub struct DeviceAttrPtr(NonNull<ibv_device_attr>);

impl DeviceAttrPtr {
    pub fn ffi_ptr(&self) -> *mut ibv_device_attr {
        self.0.as_ptr()
    }
}

#[cfg(test)]
mod tests {
    use super::{IbPortPhysState, IbPortState};

    #[test]
    fn parses_all_defined_logical_port_states() {
        let expected = [
            IbPortState::Nop,
            IbPortState::Down,
            IbPortState::Initializing,
            IbPortState::Armed,
            IbPortState::Active,
            IbPortState::ActiveDefer,
        ];

        for (value, state) in expected.into_iter().enumerate() {
            assert_eq!(IbPortState::try_from(value as u32).unwrap(), state);
        }
        assert_eq!(IbPortState::try_from(99).unwrap(), IbPortState::Unknown(99));
    }

    #[test]
    fn parses_all_defined_physical_port_states() {
        let expected = [
            IbPortPhysState::Nop,
            IbPortPhysState::Sleep,
            IbPortPhysState::Polling,
            IbPortPhysState::Disabled,
            IbPortPhysState::PortConfigurationTraining,
            IbPortPhysState::LinkUp,
            IbPortPhysState::LinkErrorRecovery,
            IbPortPhysState::PhyTest,
        ];

        for (value, state) in expected.into_iter().enumerate() {
            assert_eq!(IbPortPhysState::try_from(value as u8).unwrap(), state);
        }
        assert_eq!(
            IbPortPhysState::try_from(99).unwrap(),
            IbPortPhysState::Unknown(99)
        );
    }
}
