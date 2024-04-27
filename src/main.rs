#![no_main]
#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::mem::MaybeUninit;
use core::str::from_utf8;

use uefi::{entry, Handle, prelude::RuntimeServices, println, Status, table::runtime::VariableKey};
use uefi::table::SystemTable;
use serde::Serialize;
use uefi::prelude::Boot;
use uefi::table::runtime::VariableAttributes;
use uefi::allocator::Allocator;

#[entry]
fn main(image: Handle, mut st: SystemTable<Boot>) -> Status {
    uefi::helpers::init(&mut st)
        .expect("Unexpected error while initializing UEFI services");
    
    let rs: &RuntimeServices;
    let keys: Vec<VariableKey>;
    let mut vars: Vec<UefiVar>;
    
    //loop vars
    let mut temp_var: UefiVar = UefiVar::default();
    let mut ucsbuf: [u8; 128] = [0u8; 128];
    let mut buflen: usize;

    rs = st.runtime_services();
    keys = rs.variable_keys().expect("error getting variable keys: {}");
    vars = Vec::with_capacity(keys.len());

    for k in keys.iter() {
        buflen = ucs2::decode(
            k.name().unwrap().to_u16_slice(),
            &mut ucsbuf,
        ).expect("couldn't convert name to utf-8");
        temp_var.name = from_utf8(&ucsbuf[..buflen]).unwrap().to_string();

        let (v_data, v_attr) = rs.get_variable_boxed(k.name().unwrap(), &k.vendor)
            .expect("error getting variable: {}");

        temp_var.volatile = !v_attr.contains(VariableAttributes::NON_VOLATILE);
        temp_var.bootserivce_var = v_attr.contains(VariableAttributes::BOOTSERVICE_ACCESS);
        temp_var.bootserivce_var = v_attr.contains(VariableAttributes::RUNTIME_ACCESS);

        temp_var.data_len = v_data.len() as u64;
        temp_var.data = v_data.clone();

        // println!("{}", serde_json::to_string(&temp_var).expect("error serializing var: {}"));
        // break;
        vars.push(temp_var.clone());
    }
    
    println!("{}", serde_json::to_string(&vars).expect("error serializing vars: {}"));

    Status::SUCCESS
}

#[derive(Clone, Default, Serialize)]
struct UefiVar {
    name: String,
    vendor_guid: String,

    volatile: bool,
    bootserivce_var: bool,
    runtime_var: bool,

    data_len: u64,
    data: Box<[u8]>,
}