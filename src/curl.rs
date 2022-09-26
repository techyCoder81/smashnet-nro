use std::{io::{BufWriter, Write}, fs::File, sync::mpsc::Sender};
use skyline::libc::*;
use std::arch::asm;
use curl_sys::CURL;
use std::error::Error;
use std::path::Path;
use smashnet::types::{HttpCurlError, CurlerString};

// The following two hooks are used to manipulate the stack size of the cURL resolver thread.
// Without these hooks, the resolver thread's stack size will not be larger enough to resolve
// the URLs that we provide. We are unsure why.
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
unsafe extern "C" fn progress_callback_internal(callback: *mut ProgressCallback, dl_total: f64, dl_now: f64, ul_total: f64, ul_now: f64) -> usize {
    //println!("callback is called: {:p}", callback);
    if dl_total != 0.0 {
        ((*callback).callback)((*callback).data, dl_total, dl_now);
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
    progress_callback: Option<ProgressCallback>,
    pub curl: u64,
}

#[derive(Copy, Clone)]
struct ProgressCallback {
    callback: extern "C" fn(*mut u8, f64, f64),
    data: *mut u8,
}

// The following is the C API for using smashnet
#[export_name = "HttpCurl__new"]
pub extern "C" fn httpcurl_new(this: *mut *mut Curler) -> HttpCurlError {
    match Curler::new() {
        Ok(curler) => unsafe {
            *this = Box::leak(Box::new(curler)) as *mut Curler;
            HttpCurlError::Ok
        },
        Err(err) => err,
    }
}

#[export_name = "HttpCurl__download"]
pub extern "C" fn httpcurl_download(this: *const Curler, url: *const u8, url_len: usize, location: *const u8, location_len: usize) -> HttpCurlError {
    unsafe {
        let url = std::str::from_utf8_unchecked(std::slice::from_raw_parts(url, url_len));
        let location = std::str::from_utf8_unchecked(std::slice::from_raw_parts(location, location_len));
        match (*this).download(url, location) {
            Ok(_) => HttpCurlError::Ok,
            Err(e) => e,
        }
    }
}

#[export_name = "HttpCurl__get"]
pub extern "C" fn httpcurl_get(this: *const Curler, url: *const u8, url_len: usize, out: *mut CurlerString) -> HttpCurlError {
    unsafe {
        let url = std::str::from_utf8_unchecked(std::slice::from_raw_parts(url, url_len));
        match (*this).get(url) {
            Ok(string) => {
                let (ptr, len, cap) = string.into_raw_parts();
                std::ptr::write(out, CurlerString {
                    raw: ptr,
                    len,
                    capacity: cap
                });
                HttpCurlError::Ok
            },
            Err(e) => e
        }
    }
}

#[export_name = "HttpCurl__progress_callback"]
pub extern "C" fn httpcurl_progress_callback(this: *mut Curler, callback: extern "C" fn(*mut u8, f64, f64), data: *mut u8) -> HttpCurlError {
    unsafe {
        (*this).progress_callback(ProgressCallback {
            callback,
            data
        });
    }
    HttpCurlError::Ok
}

impl Curler {
    pub fn new() -> Result<Self, HttpCurlError> {
        install_curl();
        let curl_handle = unsafe { easy_init() };
        if !curl_handle.is_null() {
            Ok(Curler {
                progress_callback: None,
                curl: curl_handle as u64
            })
        } else {
            None
        }
    }

    /// download a file from the given url to the given location
    pub fn download<A: AsRef<str>, B: AsRef<str>>(&self, url: A, location: B) -> Result<(), HttpCurlError> {
        let url = url.as_ref();
        let location = location.as_ref();
        let temp_file = [location, ".dl"].concat();
        if Path::new(&temp_file).exists() {
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
            let cstr = [url, "\0"].concat();
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
       
            let callback = match self.progress_callback {
                Some(callback) => {
                    let callback = Box::new(callback);
                    let callback = Box::leak(callback);
                    let result = curle!(easy_setopt(curl, curl_sys::CURLOPT_NOPROGRESS, 0u64))
                        .and_then(|_| curle!(easy_setopt(curl, curl_sys::CURLOPT_PROGRESSDATA, callback as *mut ProgressCallback)))
                        .and_then(|_| curle!(easy_setopt(curl, curl_sys::CURLOPT_PROGRESSFUNCTION, progress_callback_internal as *const ())));
                    if let Err(e) = result {
                        drop(Box::from_raw(callback));
                        return Err(e);
                    }
                    callback as *mut ProgressCallback
                },
                None => {
                    curle!(easy_setopt(curl, curl_sys::CURLOPT_NOPROGRESS, 1u64))?;
                    std::ptr::null_mut()
                },
            };

            let result = curle!(easy_setopt(curl, curl_sys::CURLOPT_NOSIGNAL, 1u64))
                .and_then(|_| curle!(easy_setopt(curl, curl_sys::CURLOPT_SSL_CTX_FUNCTION, curl_ssl_ctx_callback as *const ())))
                .and_then(|_| curle!(easy_setopt(curl, curl_sys::CURLOPT_USERAGENT, "smashnet\0".as_ptr())));
            
            if let Err(e) = result {
                if !callback.is_null() {
                    drop(Box::from_raw(callback));
                }
                return Err(e);
            }

            println!("beginning download.");
            match curle!(easy_perform(curl)){
                Ok(()) => println!("curl success?"),
                Err(e) => println!("Error during curl: {}", e) 
            };

            drop(Box::from_raw(callback));
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

    /// GET text from the given url
    pub fn get<S: AsRef<str>>(&mut self, url: S) -> Result<String, HttpCurlError>{
        let tick = unsafe {skyline::nn::os::GetSystemTick() as usize};
        let location = format!("sd:/downloads/{}.txt", tick);
        match self.download(url, &location) {
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

    pub fn progress_callback(&mut self, callback: ProgressCallback) -> &mut Self {
        self.progress_callback = Some(callback);
        self
    }
}

impl Drop for Curler {
    #[export_name = "Curler__drop"]
    extern "C" fn drop(&mut self) {
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
        if let Some(callback) = self.progress_callback.take() {
            let closure = callback.data as *mut Box<dyn FnMut(f64, f64) + 'static>;
            unsafe {
                drop(Box::from_raw(closure))
            }
        }
    }
}

// Used to only install the stack size hooks once
static INSTALL: std::sync::Once = std::sync::Once::new();

/// Installs the required hooks for cURL
pub fn install_curl() {
    INSTALL.call_once(|| {
        skyline::install_hooks!(
            libcurl_resolver_thread_stack_size_set,
            libcurl_resolver_thread_stack_size_set2
        );
    });
}