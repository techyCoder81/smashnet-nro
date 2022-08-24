#![feature(proc_macro_hygiene)]
#![feature(allocator_api)]
//#![feature(asm)]
#![allow(non_camel_case_types)]
#![allow(unused_imports)]
#![allow(dead_code)]
#![feature(c_variadic)]
mod curl;

#[skyline::main(name = "smashnet-nro")]
pub fn main() {
    curl::install_curl();
}
