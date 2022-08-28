#![feature(proc_macro_hygiene)]
#![feature(allocator_api)]
//#![feature(asm)]
#![allow(non_camel_case_types)]
#![allow(unused_imports)]
#![allow(dead_code)]
#![feature(c_variadic)]
mod curl;
use curl::*;
use smashnet::*;

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
        .download("https://github.com/techyCoder81/smashnet-nro/releases/download/nightly/checksum.txt", "sd:/downloads/checksum.txt") {
            Ok(_) => println!("download was ok!"),
            Err(e) => println!("Error during download: {}", e)
    }

    let latest_hash = fs::read_to_string("sd:/downloads/checksum.txt");
    println!("Hash of latest: {}", latest_hash);
    
    // read the file
    let data = match fs::read("sd:/atmosphere/contents/01006A800016E000/romfs/skyline/plugins/libsmashnet.nro") {
        Ok(bytes) => bytes,
        Err(e) => {println!("error during smashnet update!"); return;}
    };
    // compute the md5 and return the value
    let digest = md5::compute(data);
    let current_hash = format!("{:x}", digest);
    println!("hash of current install: {:x}", current_hash);
    
    if (current_hash != latest_hash) {
        let should_update = skyline_web::Dialog::yes_no("An update is available for smashnet.nro! Would you like to update?");
        if should_update {
            println!("updating smashnet!");
            Curler::new().download(
                "https://github.com/techyCoder81/smashnet-nro/releases/download/nightly/libsmashnet.nro", 
                "sd:/atmosphere/contents/01006A800016E000/romfs/skyline/plugins/libsmashnet.nro");
            } else {
                println!("not updating smashnet!");
            }
    }
}
