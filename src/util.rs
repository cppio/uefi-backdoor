use core::{
    mem::{self, MaybeUninit},
    ops::Range,
    slice,
};

use r_efi::{protocols::simple_text_output, system::RuntimeServices as RawRuntimeServices};
use uefi::{prelude::*, proto::console::text::Color, Completion};

static mut SYSTEM_TABLE: MaybeUninit<SystemTable<Boot>> = MaybeUninit::uninit();

pub fn system_table() -> &'static SystemTable<Boot> {
    unsafe { &*SYSTEM_TABLE.as_ptr() }
}

pub fn raw_runtime_services() -> &'static RawRuntimeServices {
    unsafe { &*(system_table().runtime_services() as *const _ as *const _) }
}

macro_rules! print {
    ($($arg:tt)*) => { {
        use ::core::fmt::Write;
        let _ = ::core::write!($crate::util::system_table().stdout(), $($arg)*);
    } }
}

macro_rules! println {
    ($($arg:tt)*) => { {
        use ::core::fmt::Write;
        let _ = ::core::writeln!($crate::util::system_table().stdout(), $($arg)*);
    } }
}

#[entry]
fn efi_main(_image_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    unsafe { SYSTEM_TABLE = MaybeUninit::new(system_table) };

    main();

    Status::LOAD_ERROR
}

fn main() {
    let stdout = system_table().stdout();

    let (foreground, background) = unsafe {
        let raw_stdout = &*(stdout as *const _ as *const simple_text_output::Protocol);
        let mode = &*raw_stdout.mode;
        mem::transmute((
            (mode.attribute & 0xF) as u8,
            (mode.attribute >> 4 & 0x7) as u8,
        ))
    };

    match crate::main() {
        Status::SUCCESS => {
            let _ = stdout.set_color(Color::LightGreen, background);
            println!("╔══════════╗");
            println!("║ Success! ║");
            println!("╚══════════╝");
        }
        status => {
            let _ = stdout.set_color(Color::LightRed, background);
            println!("[-] error: {:?}", status);
        }
    }

    let _ = stdout.set_color(Color::White, background);
    print!("Press any key to continue...");
    let _ = stdout.set_color(foreground, background);

    let stdin = system_table().stdin();
    let _ = system_table()
        .boot_services()
        .wait_for_event(&mut [stdin.wait_for_key_event()]);
    let _ = stdin.read_key();

    println!();
}

macro_rules! unwrap {
    ($expr:expr) => {
        $expr?.split().1
    };
}

static mut BUFFER: [u8; 4096] = [0; 4096];

pub fn region_containing(address: usize) -> uefi::Result<Range<usize>> {
    let (status, (_, descriptors)) = system_table()
        .boot_services()
        .memory_map(unsafe { &mut BUFFER })?
        .split();

    let region = descriptors
        .map(|descriptor| {
            let start = descriptor.phys_start as usize;
            let end = start + descriptor.page_count as usize * 4096;

            start..end
        })
        .find(|region| region.contains(&address));

    match region {
        Some(region) => Ok(Completion::new(status, region)),
        None => Err(Status::NOT_FOUND.into()),
    }
}

pub unsafe fn range_to_slice(range: Range<usize>) -> &'static mut [u8] {
    slice::from_raw_parts_mut(range.start as _, range.len())
}

pub fn search_for_contiguous(slice: &mut [u8], item: u8, count: usize) -> uefi::Result<&mut [u8]> {
    let mut current = 0;

    for (n, i) in slice.iter().enumerate() {
        if *i == item {
            current += 1;

            if current == count {
                let slice = &mut slice[n + 1 - count..n + 1];

                return Ok(slice.into());
            }
        } else if current != 0 {
            current = 0;
        }
    }

    Err(Status::NOT_FOUND.into())
}
