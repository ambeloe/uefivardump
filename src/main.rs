#![no_main]
#![no_std]

extern crate alloc;

use alloc::boxed::Box;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use core::ops::BitAnd;
use core::str::from_utf8;
use core::usize;

use serde::Serialize;
use core::panic::PanicInfo;

use uefi::{Char16, cstr16, CStr16, entry, Handle, prelude::RuntimeServices, print, println, Status, table::runtime::VariableKey};


use uefi::prelude::{Boot, SystemTable};
use uefi::proto::loaded_image::LoadedImage;
use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode};

use uefi::table::runtime::{ResetType, VariableAttributes, VariableVendor};
use getargs::{Arg, Options};



static mut ST_PTR: *mut SystemTable<Boot> = core::ptr::null_mut();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[FUCK] {}", info);

    unsafe {
        let st = &mut *ST_PTR;

        st.boot_services().exit(st.boot_services().image_handle(), Status::ABORTED, 0, &mut Char16::from_u16_unchecked(0))
    }
}

#[entry]
unsafe fn main(image: Handle, mut st: SystemTable<Boot>) -> Status {
    let _ = uefi::helpers::init(&mut st);
    // .expect("Unexpected error while initializing UEFI services");
    // handling the panic is pointless without the handler working

    //give panic handler access to system table
    unsafe {
        ST_PTR = &mut st as *mut SystemTable<Boot>;
    }

    let args: Vec<String>;
    let rs: &RuntimeServices;
    let keys: Vec<VariableKey>;
    let mut u16buf: [u16; 0xfff] = [0u16; 0xfff];
    let mut json: String;

    macro_rules! open_file {
        ($file: expr, $mode: expr) => {
            st.boot_services().get_image_file_system(st.boot_services()
                .image_handle()).expect("error getting current image fs")
                .open_volume().expect("error opening root directory on volume")
                .open(string_to_cstr16($file, &mut u16buf), $mode, FileAttribute::empty()).expect("failed to open file")
                .into_regular_file().expect("file can't be a directory")
        };
    }
    macro_rules! seek_end {
        ($file: expr) => {
            let flen = $file.get_boxed_info::<FileInfo>().expect("error getting file info").file_size();
            $file.set_position(flen).expect("error seeking");
        };
    }

    //options
    let mut volatility_filter: Filter = Filter::KeepAll;
    let mut output_file: String = "-".to_string();
    let mut reboot: bool = false;

    //loop vars
    let mut temp_var: UefiVar = UefiVar::default();
    let mut ucsbuf: [u8; 0xfff] = [0u8; 0xfff];

    rs = st.runtime_services();
    
    // todo: handle spaces in filename
    args = cstr16_to_string(
        st.boot_services()
            .open_protocol_exclusive::<LoadedImage>(st.boot_services().image_handle()).expect("error getting handle to current image")
            .load_options_as_cstr16().expect("error loading options"), &mut ucsbuf).expect("error decoding ucs2")
        .split(" ").map(|s| s.to_string()).collect();

    let mut opts = Options::new(args.iter().skip(1).map(String::as_str));
    while let Some(arg) = opts.next_arg().expect("argument parsing error") {
        match arg {
            Arg::Short('r') => {
                reboot = true;
            }
            Arg::Short('v') => {
                let filterstr = opts.value_opt().expect("no filter type specified");
                match filterstr {
                    "true" => { volatility_filter = Filter::KeepVolatile }
                    "false" => { volatility_filter = Filter::KeepPersistent }
                    _ => {
                        println!("{} is not a valid option for -v", filterstr);
                        return Status::ABORTED;
                    }
                }
            }
            Arg::Short('f') => {
                output_file = opts.value_opt().expect("filename cannot be empty!").to_string();
            }
            Arg::Short('h') | Arg::Short('?') | Arg::Long("help") => {
                println!(r"
                uefivardumper takes three optional parameters: -v[true/false], -r,and -f.
                    -v specifies if the saved variables are volatile or not (-vtrue only saves voltatile, -vfalse only saves persistent)
                        not specifying the variable will save all variables.
                    -f specifies the output filename (path is relative to the drive uefivardumper is stored on) defaults to - for stdout
                    example: uefivardump.efi -vtrue -ftest.json
                    -r reboot to uefi after dump is finished
                ");
                return Status::SUCCESS;
            }

            _ => {
                println!("Invalid argument passed: {}", arg.to_string());
                return Status::ABORTED;
            }
        }
    }

    if output_file != "-" {
        let mut file = open_file!(&output_file, FileMode::CreateReadWrite);
        
        file.write("[".as_bytes()).expect("error writing to file");
        file.flush().expect("error flushing buffer to file (beginning)");
        file.close();
    } else {
        print!("[");
    }

    keys = rs.variable_keys().expect("error getting variable keys: {}");
    for k in keys.iter() {
        // println!("LOOP");
        temp_var.name = cstr16_to_string(k.name().unwrap(), &mut ucsbuf).expect("couldn't convert name to utf-8");

        let (v_data, v_attr) = rs.get_variable_boxed(k.name().unwrap(), &k.vendor)
            .expect("error getting variable: {}");

        temp_var.volatile = !v_attr.contains(VariableAttributes::NON_VOLATILE);
        temp_var.bootserivce_var = v_attr.contains(VariableAttributes::BOOTSERVICE_ACCESS);
        temp_var.bootserivce_var = v_attr.contains(VariableAttributes::RUNTIME_ACCESS);

        temp_var.data_len = v_data.len() as u64;
        temp_var.data = v_data.clone();

        match volatility_filter {
            Filter::KeepAll => {}
            Filter::KeepVolatile => { if !temp_var.volatile { continue; } }
            Filter::KeepPersistent => { if temp_var.volatile { continue; } }
        }

        json = serde_json::to_string(&temp_var).expect("error serializing var");
        if output_file != "-" {
            let mut file = open_file!(&output_file, FileMode::ReadWrite);
            seek_end!(file);

            file.write(json.as_bytes()).expect("error writing var to file");
            file.write(",".as_bytes()).expect("error writing comma to file");
            file.flush().expect("error flushing file buffer (argdump)");
            file.close()
        } else {
            print!("{},", json);
        }
    }

    if output_file != "-" {
        let mut file = open_file!(&output_file, FileMode::ReadWrite);
        seek_end!(file);

        file.write("]".as_bytes()).expect("error writing ending bracket to file");
        file.flush().expect("error flushing file buffer (final)");
        file.close()
    } else {
        println!("]");
    }
    
    if reboot {
        //skip if not supported
        if rs.get_variable_boxed(cstr16!("OsIndications"), &VariableVendor::GLOBAL_VARIABLE).unwrap_or((Box::new([0u8]), VariableAttributes::default())).0[0].bitand(1) > 0 {
            let mut ex = rs.get_variable_boxed(cstr16!("OsIndications"), &VariableVendor::GLOBAL_VARIABLE).expect("could not read variable");
            ex.0[0] |= 1;
            rs.set_variable(cstr16!("OsIndications"), &VariableVendor::GLOBAL_VARIABLE, ex.1, ex.0.as_ref()).expect("couldn't set variable");
        }
        rs.reset(ResetType::WARM, Status::SUCCESS, None);
    }

    Status::SUCCESS
}

fn cstr16_to_string(c: &CStr16, buf: &mut [u8]) -> Result<String, ucs2::Error> {
    let len: usize = ucs2::decode(
        c.to_u16_slice(),
        buf,
    )?;
    Ok(from_utf8(&buf[..len]).unwrap().to_string())
}


fn string_to_cstr16<'a>(s: &String, buf: &'a mut [u16]) -> &'a CStr16 {
    let len: usize = ucs2::encode(s, buf).expect("error encoding to ucs2");
    if len >= buf.len() {
        panic!("buffer too small to fit extra null byte {} vs {}", len, buf.len());
    }
    buf[len] = 0u16; //add null byte
    CStr16::from_u16_with_nul(&buf[..len + 1]).expect("error encoding to ")
}

enum Filter {
    KeepAll,
    KeepVolatile,
    KeepPersistent,
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