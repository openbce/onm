use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rustc-link-lib=pci");
    println!("cargo:rustc-link-lib=ibverbs");
    println!("cargo:rerun-if-changed=wrappers/ib.h");
    println!("cargo:rerun-if-changed=wrappers/pci.h");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    let bindings = bindgen::Builder::default()
        .header("wrappers/ib.h")
        .blocklist_type("u8")
        .blocklist_type("u16")
        .blocklist_type("u32")
        .blocklist_type("u64")
        .bitfield_enum("ibv_access_flags")
        .bitfield_enum("ibv_qp_attr_mask")
        .bitfield_enum("ibv_wc_flags")
        .bitfield_enum("ibv_send_flags")
        .bitfield_enum("ibv_port_cap_flags")
        .constified_enum_module("ibv_qp_type")
        .constified_enum_module("ibv_qp_state")
        .constified_enum_module("ibv_port_state")
        .constified_enum_module("ibv_wc_opcode")
        .constified_enum_module("ibv_wr_opcode")
        .constified_enum_module("ibv_wc_status")
        .derive_default(true)
        .derive_debug(true)
        .prepend_enum_name(false)
        .size_t_is_usize(true)
        .generate_comments(true)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate ib bindings");

    bindings
        .write_to_file(out_path.join("ib.rs"))
        .expect("Couldn't write ib bindings!");

    let bindings = bindgen::Builder::default()
        .header("wrappers/pci.h")
        .blocklist_type("u8")
        .blocklist_type("u16")
        .blocklist_type("u32")
        .blocklist_type("u64")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate pci bindings");

    bindings
        .write_to_file(out_path.join("pci.rs"))
        .expect("Couldn't write pci bindings!");
}
