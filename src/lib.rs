#![feature(proc_macro_hygiene)]
#![feature(allocator_api)]
//#![feature(asm)]
#![allow(non_camel_case_types)]
#![allow(unused_imports)]
#![allow(dead_code)]
#![feature(c_variadic)]
mod curl;

pub fn is_emulator() -> bool {
    return unsafe { skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as u64 } == 0x8004000;
}

#[skyline::main(name = "smashnet-nro")]
pub fn main() {
    curl::install_curl();
    if is_emulator() {
        println!("not checking for updates for smashnet since we are on emulator.");
        return;
    }
    // check for updates here
}
