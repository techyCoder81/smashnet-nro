#![feature(proc_macro_hygiene)]
#![feature(allocator_api)]
//#![feature(asm)]
#![allow(non_camel_case_types)]
#![allow(unused_imports)]
#![allow(dead_code)]
#![feature(c_variadic)]
mod curl;
mod request;
use curl::*;
use request::*;
use std::fs;
use smashnet::HttpCurl;

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
    println!("checking for smashnet updates...");
    match Curler::new()
        //.progress_callback(|total, current| session.progress(current/total, &self.id))
        .download(
            "https://github.com/techyCoder81/smashnet-nro/releases/download/nightly/checksum.txt".to_string(), 
        "sd:/downloads/checksum.txt".to_string()) {
            Ok(_) => println!("download was ok!"),
            Err(e) => {println!("Error during download: {}", e); return;}
    }

    let contents = match fs::read_to_string("sd:/downloads/checksum.txt"){
        Ok(hash) => hash,
        Err(e) => {println!("Error reading downloaded hash file: {}", e); return;}
    };
    let latest_hash = contents.split(" ").next().unwrap().clone();
    println!("Hash of latest: {}", latest_hash);
    
    // read the file
    let data = match fs::read("sd:/atmosphere/contents/01006A800016E000/romfs/skyline/plugins/libsmashnet.nro") {
        Ok(bytes) => bytes,
        Err(e) => {println!("error during smashnet update!"); return;}
    };
    // compute the md5 and return the value
    let digest = md5::compute(data);
    let current_hash = format!("{:x}", digest);
    println!("hash of current install: {}", current_hash);
    
    if current_hash != latest_hash {
        println!("asking if we want to update smashnet");
        let should_update = skyline_web::Dialog::yes_no("An update is available for smashnet.nro! Would you like to update?");
        if should_update {
            println!("updating smashnet!");
            Curler::new().download(
                "https://github.com/techyCoder81/smashnet-nro/releases/download/nightly/libsmashnet.nro".to_string(), 
                "sd:/atmosphere/contents/01006A800016E000/romfs/skyline/plugins/libsmashnet.nro".to_string());
            } else {
                println!("not updating smashnet!");
            }
    } else {
        println!("smashnet is up to date!");
    }
}
