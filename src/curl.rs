use std::{io::{BufWriter, Write}, fs::File, sync::mpsc::Sender};
use skyline::libc::*;
use std::arch::asm;
use curl_sys::CURL;
use std::error::Error;
use std::path::Path;
use smashnet::HttpCurl;

#[skyline::hook(offset = 0x6aa8, inline)]
pub unsafe fn curl_log_hook(ctx: &skyline::hooks::InlineCtx) {
    let str_ptr;
    asm!("ldr {}, [x29, #0x18]", out(reg) str_ptr);
    println!("{}", skyline::from_c_str(str_ptr));
}

#[skyline::hook(offset = 0x27ac0, inline)]
pub unsafe fn libcurl_resolver_thread_stack_size_set(ctx: &mut skyline::hooks::InlineCtx) {
    *ctx.registers[1].x.as_mut() = 0x10_000;
}

#[skyline::hook(offset = 0x27af4, inline)]
pub unsafe fn libcurl_resolver_thread_stack_size_set2(ctx: &mut skyline::hooks::InlineCtx) {
    *ctx.registers[4].x.as_mut() = 0x10_000;
}


#[skyline::from_offset(0x7f0)]
pub unsafe extern "C" fn global_init_mem(
    init_args: u64,
    malloc: unsafe extern "C" fn(usize) -> *mut c_void,
    free: unsafe extern "C" fn(*mut c_void),
    realloc: unsafe extern "C" fn(*mut c_void, usize) -> *mut c_void,
    strdup: unsafe extern "C" fn(*const u8) -> *mut u8,
    calloc: unsafe extern "C" fn(usize, usize) -> *mut c_void
) -> curl_sys::CURLcode;

#[skyline::from_offset(0x16c00)]
pub unsafe extern "C" fn slist_append(slist: *mut curl_sys::curl_slist, header: *const u8) -> *mut curl_sys::curl_slist;

#[skyline::from_offset(0x960)]
pub unsafe extern "C" fn easy_init() -> *mut curl_sys::CURL;

#[skyline::from_offset(0xA00)]
pub unsafe extern "C" fn easy_setopt(curl: *mut curl_sys::CURL, option: curl_sys::CURLoption, ...) -> curl_sys::CURLcode;

#[skyline::from_offset(0xA90)]
pub unsafe extern "C" fn easy_perform(curl: *mut curl_sys::CURL) -> curl_sys::CURLcode;

#[skyline::from_offset(0xC70)]
pub unsafe extern "C" fn easy_cleanup(curl: *mut curl_sys::CURL) -> curl_sys::CURLcode;

#[skyline::from_offset(0x36f6d40)]
pub unsafe extern "C" fn curl_global_malloc(size: usize) -> *mut u8;

#[skyline::from_offset(0x36f6dc0)]
pub unsafe extern "C" fn curl_global_free(ptr: *mut u8);

#[skyline::from_offset(0x36f6e40)]
pub unsafe extern "C" fn curl_global_realloc(ptr: *mut u8, size: usize) -> *mut u8;

#[skyline::from_offset(0x36f6ec0)]
pub unsafe extern "C" fn curl_global_strdup(ptr: *const u8) -> *mut u8;

#[skyline::from_offset(0x36f6fa0)]
pub unsafe extern "C" fn curl_global_calloc(nmemb: usize, size: usize) -> *mut u8;

#[skyline::from_offset(0x21fd50)]
pub unsafe extern "C" fn curl_ssl_ctx_callback(arg1: u64, arg2: u64, arg3: u64) -> curl_sys::CURLcode;

unsafe extern "C" fn write_fn(data: *const u8, data_size: usize, data_count: usize, writer: &mut BufWriter<File>) -> usize {
    let true_size = data_size * data_count;
    let slice = std::slice::from_raw_parts(data, true_size);
    let _ = writer.write(slice);
    true_size
}

/// private internal callback handler
unsafe extern "C" fn callback_internal(callback: extern fn(f64, f64) -> (), dl_total: f64, dl_now: f64, ul_total: f64, ul_now: f64) -> usize {
    //println!("callback is called: {:p}", callback);
    if dl_total != 0.0 {
        callback(dl_total, dl_now);
    }
    0
}

macro_rules! curle {
    ($e:expr) => {{
        let result = $e;
        if result != ::curl_sys::CURLE_OK {
            Err(result)
        } else {
            Ok(())
        }
    }}
}

/// this MUST align withe HttpCurl defined in the smashnet main package!
pub struct Curler {
    pub callback: Option<fn(f64, f64) -> ()>,
    pub curl: u64,
}

impl HttpCurl for Curler {

    #[export_name = "HttpCurl__new"]
    extern "Rust" fn new() -> Self { 
        install_curl();
        let curl_handle = unsafe { easy_init() };
        return Curler{callback: None, curl: curl_handle as u64};
    }
    #[export_name = "HttpCurl__is_valid"]
    extern "Rust" fn is_valid(&mut self) -> Result<&mut Curler, u64> {
        let curl = self.curl as *mut CURL;
        if curl.is_null() {
            let error = format!("curl failed to initialize! Pointer value: {:p}", curl);
            println!("{}", error);
            return Err(self.curl);
        }
        return Ok(self);
    }

    /// download a file from the given url to the given location
    #[export_name = "HttpCurl__download"]
    extern "Rust" fn download(&mut self, url: String, location: String) -> Result<(), u32>{
        // change thread to high priority
        //unsafe {
        //    skyline::nn::os::ChangeThreadPriority(skyline::nn::os::GetCurrentThread(), 2);
        //}

        // temp file name: myfile.txt.dl
        let temp_file = [location.as_str(), ".dl"].concat();
        if Path::new(temp_file.as_str()).exists() {
            println!("removing existing temp file: {}", temp_file);
            std::fs::remove_file(&temp_file);
        }

        println!("creating temp file: {}", temp_file);
        let mut writer = std::io::BufWriter::with_capacity(
            0x40_0000,
            std::fs::File::create(&temp_file).unwrap()
        );
        println!("created bufwriter with capacity");
        unsafe {
            let cstr = [url.as_str(), "\0"].concat();
            let ptr = cstr.as_str().as_ptr();
            let curl = self.curl as *mut CURL;
            println!("curl is initialized, beginning options");
            //let header = slist_append(std::ptr::null_mut(), "Accept: application/octet-stream\0".as_ptr());
            curle!(easy_setopt(curl, curl_sys::CURLOPT_URL, ptr))?;
            //curle!(easy_setopt(curl, curl_sys::CURLOPT_HTTPHEADER, header))?;
            curle!(easy_setopt(curl, curl_sys::CURLOPT_FOLLOWLOCATION, 1u64))?;
            curle!(easy_setopt(curl, curl_sys::CURLOPT_WRITEDATA, &mut writer))?;
            curle!(easy_setopt(curl, curl_sys::CURLOPT_WRITEFUNCTION, write_fn as *const ()))?;
            curle!(easy_setopt(curl, curl_sys::CURLOPT_FAILONERROR, 1u64))?;
       
            match self.callback {
                Some(function) => {
                    curle!(easy_setopt(curl, curl_sys::CURLOPT_NOPROGRESS, 0u64))?;
                    curle!(easy_setopt(curl, curl_sys::CURLOPT_PROGRESSDATA, function as *const ()))?;
                    curle!(easy_setopt(curl, curl_sys::CURLOPT_PROGRESSFUNCTION, callback_internal as *const ()))?;
                },
                None => curle!(easy_setopt(curl, curl_sys::CURLOPT_NOPROGRESS, 1u64))?,
            }
            curle!(easy_setopt(curl, curl_sys::CURLOPT_NOSIGNAL, 1u64))?;
            curle!(easy_setopt(curl, curl_sys::CURLOPT_SSL_CTX_FUNCTION, curl_ssl_ctx_callback as *const ()))?;
            curle!(easy_setopt(curl, curl_sys::CURLOPT_USERAGENT, "smashnet\0".as_ptr()))?;
            println!("beginning download.");
            match curle!(easy_perform(curl)){
                Ok(()) => println!("curl success?"),
                Err(e) => println!("Error during curl: {}", e) 
            };
        }

        println!("flushing writer");
        writer.flush();
        println!("dropping writer");
        std::mem::drop(writer);

        if std::fs::metadata(&temp_file.as_str()).unwrap().len() < 8 {
            // empty files should be considered an error.
            println!("File was empty, assuming failure.");
            std::fs::remove_file(&temp_file);
            return Err(0);
        }

        // replace/rename the temp file to the expected location
        if Path::new(location.as_str()).exists() {
            println!("removing original path: {}", location);
            std::fs::remove_file(location.as_str());
        }
        std::fs::rename(&temp_file, location);

        //println!("resetting priority of thread");
        //unsafe {
        //    skyline::nn::os::ChangeThreadPriority(skyline::nn::os::GetCurrentThread(), 16);
        //}
        println!("download complete.");
        Ok(())
    }

    /// GET json from the given url
    #[export_name = "HttpCurl__get_json"]
    extern "Rust" fn get_json(&mut self, url: String) -> Result<String, String>{
        let tick = unsafe {skyline::nn::os::GetSystemTick() as usize};
        fs::create_dir_all("sd:/downloads")?;
        let location = format!("sd:/downloads/{}.json", tick);
        match self.download(url, location.clone()) {
            Ok(()) => println!("json GET ok!"),
            Err(e) => {
                let error = format!("{}", e);
                return Err(error);
            }
        }
        let json = match std::fs::read_to_string(&location){
            Ok(text) => text,
            Err(e) => {
                let error = format!("{}", e);
                return Err(error);
            }
        };
        std::fs::remove_file(&location);
        return Ok(json);
    }

    /// GET text from the given url
    #[export_name = "HttpCurl__get"]
    extern "Rust" fn get(&mut self, url: String) -> Result<String, String>{
        let tick = unsafe {skyline::nn::os::GetSystemTick() as usize};
        fs::create_dir_all("sd:/downloads")?;
        let location = format!("sd:/downloads/{}.txt", tick);
        match self.download(url, location.clone()) {
            Ok(()) => println!("text GET ok!"),
            Err(e) => {
                let error = format!("{}", e);
                return Err(error);
            }
        }
        let str = match std::fs::read_to_string(&location){
            Ok(text) => text,
            Err(e) => {
                let error = format!("{}", e);
                return Err(error);
            }
        };
        std::fs::remove_file(&location);
        return Ok(str);
    }

    #[export_name = "HttpCurl__progress_callback"]
    extern "Rust" fn progress_callback(&mut self, function: fn(f64, f64) -> ()) -> &mut Self {
        self.callback = Some(function);
        self
    }

    
}

impl Drop for Curler {
    #[export_name = "Curler__drop"]
    extern "Rust" fn drop(&mut self) {
        let curl = self.curl as *mut CURL;
        if !curl.is_null() {
            println!("cleaning up curl handle from curler.");
            unsafe { 
                match curle!(easy_cleanup(curl)) {
                    Ok(_) => println!("cleaned up curl successfully."),
                    Err(e) => println!("cleaning up curl failed with error code: {}", e),
                }; 
            }
        }
    }
}

static mut INSTALLED: bool = false;

pub fn install_curl() {
    unsafe {
        if !INSTALLED {
            INSTALLED = true;
            skyline::install_hooks!(
                //curl_log_hook,
                libcurl_resolver_thread_stack_size_set,
                libcurl_resolver_thread_stack_size_set2
            );
        }
    }
}