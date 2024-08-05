#![no_main]
#![no_std]

mod types;

extern crate alloc;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ops::{BitAnd, Deref};
use core::str::from_utf8;
use core::usize;
use core::panic::PanicInfo;
use core::slice::{from_raw_parts, from_raw_parts_mut};
use uefi::{Char16, cstr16, CStr16, entry, Guid, Handle, prelude::RuntimeServices, print, println, Status, table::runtime::VariableKey};
use uefi::prelude::{Boot, SystemTable};
use uefi::proto::loaded_image::LoadedImage;
use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode, RegularFile};
use uefi::table::runtime::{ResetType, VariableAttributes, VariableVendor};
use getargs::{Arg, Options};
use uefi::table::boot::MemoryType;
use crate::types::{UefiVar, VarAttributes};


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
fn main(image: Handle, mut st: SystemTable<Boot>) -> Status {
    //give panic handler access to system table
    unsafe {
        ST_PTR = &mut st as *mut SystemTable<Boot>;
    }

    let _ = uefi::helpers::init(&mut st).expect("Unexpected error while initializing UEFI services");


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
    let mut output_file: String = "".to_string();
    let mut dry_run: bool = false;
    let mut reboot: bool = false;
    let mut write: bool = false;

    let mut fbuf: Option<*mut u8> = None;

    //loop vars
    let mut first: bool = true;
    let mut temp_var: UefiVar = Default::default();
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
            Arg::Short('w') => {
                write = true;
            }
            Arg::Short('d') => {
                dry_run = true;
            }
            //? won't be used because of efishell help hijacking it but might as well keep it
            Arg::Short('h') | Arg::Short('?') | Arg::Long("help") => {
                println!(r"
                uefivardumper takes five optional parameters: -v[true/false], -r, -f[filename], -d, and -w.
                    -v specifies if the saved/written variables are volatile or not (-vtrue only saves/writes voltatile, -vfalse only saves/writes persistent)
                        not specifying the variable will save all variables.
                    -f specifies the dump filename (path is relative to the drive uefivardumper is stored on) defaults to - for stdout
                    -r reboot to uefi after dump is finished
                    -w writes vars in dump to uefi (-f must be specified)
                    -d dry run (does everything but actually write the variable and restart)");
                return Status::SUCCESS;
            }

            _ => {
                println!("Invalid argument passed: {}", arg.to_string());
                return Status::ABORTED;
            }
        }
    }

    if write {
        if output_file == "-" {
            println!("write must be used with the -f argument!");
            return Status::ABORTED;
        }

        let mut file: RegularFile = open_file!(&output_file, FileMode::Read);
        let flen: usize = file.get_boxed_info::<FileInfo>().expect("failed to get file info").file_size() as usize;
        fbuf = Some(st.boot_services().allocate_pool(MemoryType::BOOT_SERVICES_DATA, flen).expect("failed to allocate file buffer"));
        let fstr: &str;

        unsafe {
            file.read(from_raw_parts_mut(fbuf.unwrap(), flen)).expect("failed to read file into buf");
            fstr = from_utf8(from_raw_parts(fbuf.unwrap(), flen)).expect("failed to create string from file (invalid utf8)");
        }

        let vars: Vec<UefiVar> = serde_json::from_str(fstr).expect("failed to unmarshal json (possible dump version mismatch)");

        for var in vars {
            if var.attributes.time_based_authenticated_write_access || var.attributes.time_based_authenticated_write_access || var.attributes.enhanced_authenticated_access {
                println!("skipping {} because it needs authentication", var.name)
            } else {
                if !dry_run {
                    match volatility_filter {
                        Filter::KeepAll => {}
                        Filter::KeepVolatile => { if temp_var.attributes.non_volatile { continue; } }
                        Filter::KeepPersistent => { if !temp_var.attributes.non_volatile { continue; } }
                    }

                    match rs.set_variable(
                        string_to_cstr16(&var.name, &mut u16buf),
                        &VariableVendor(Guid::parse_or_panic(var.vendor_guid.as_str())),
                        var.attributes.into(),
                        var.data.deref(),
                    ) {
                        Ok(_) => println!("wrote \"{}\" {}b", var.name, var.data_len),
                        Err(s) => match s.status() {
                            Status::WRITE_PROTECTED => println!("failed to write \"{}\" because it was write protected", var.name),
                            _ => panic!("failed to set variable {}: {}", var.name, s)
                        }
                    }
                } else {
                    println!("dry run: skipping write of {}", var.name)
                }
            }
        }
    } else {
        if output_file == "" {
            let time = rs.get_time().expect("error getting time");
            output_file = format!("uefivardump{:04}-{:02}-{:02}T{:02}_{:02}_{:02}.json", time.year(), time.month(), time.day(), time.hour(), time.minute(), time.second());
        }
        // println!("{}", output_file);

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
            temp_var.vendor_guid = k.vendor.0.to_string();

            let (v_data, v_attr) = rs.get_variable_boxed(k.name().unwrap(), &k.vendor)
                .expect("error getting variable: {}");

            temp_var.attributes = VarAttributes::from(v_attr);

            temp_var.data_len = v_data.len() as u64;
            temp_var.data = v_data.clone();

            match volatility_filter {
                Filter::KeepAll => {}
                Filter::KeepVolatile => { if temp_var.attributes.non_volatile { continue; } }
                Filter::KeepPersistent => { if !temp_var.attributes.non_volatile { continue; } }
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
                if first {
                    print!("{}", json);
                    first = false;
                } else {
                    print!(",{}", json);
                }
            }
        }

        if output_file != "-" {
            let mut file = open_file!(&output_file, FileMode::ReadWrite);
            let flen = file.get_boxed_info::<FileInfo>().expect("error getting file info").file_size();
            file.set_position(flen - 1).expect("error seeking");

            file.write("]".as_bytes()).expect("error writing ending bracket to file");
            file.flush().expect("error flushing file buffer (final)");
            file.close()
        } else {
            println!("]");
        }
    }

    if fbuf.is_some() {
        unsafe {
            st.boot_services().free_pool(fbuf.unwrap()).expect("failed to free file buffer");
        }
    }

    if reboot {
        if !dry_run {
            //skip if not supported
            if rs.get_variable_boxed(cstr16!("OsIndicationsSupported"), &VariableVendor::GLOBAL_VARIABLE).unwrap_or((Box::new([0u8]), VariableAttributes::default())).0[0].bitand(1) > 0 {
                let mut ex = rs.get_variable_boxed(cstr16!("OsIndications"), &VariableVendor::GLOBAL_VARIABLE).expect("could not read variable");
                ex.0[0] |= 1;
                rs.set_variable(cstr16!("OsIndications"), &VariableVendor::GLOBAL_VARIABLE, ex.1, ex.0.as_ref()).expect("couldn't set variable");
            }
            rs.reset(ResetType::WARM, Status::SUCCESS, None);
        } else {
            println!("dry run: skipping reboot")
        }
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