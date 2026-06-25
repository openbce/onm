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

use std::ffi::{c_char, CStr};
use std::io;

use libudev::Device;

pub unsafe fn cstr_to_string(s: *const c_char) -> String {
    CStr::from_ptr(s)
        .to_str()
        .expect("not an utf8 string")
        .to_string()
}

pub fn get_property<'a>(device: &'a Device, name: &'a str) -> io::Result<&'a str> {
    match device.property_value(name) {
        None => Err(io::Error::last_os_error()),
        Some(p) => p
            .to_str()
            .map(|s| s.trim())
            .ok_or_else(io::Error::last_os_error),
    }
}

pub fn get_sysattr<'a>(device: &'a Device, name: &'a str) -> io::Result<&'a str> {
    match device.attribute_value(name) {
        None => Err(io::Error::last_os_error()),
        Some(p) => p
            .to_str()
            .map(|s| s.trim())
            .ok_or_else(io::Error::last_os_error),
    }
}
