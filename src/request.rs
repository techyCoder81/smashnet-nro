
#[export_name = "Smashnet__get"]
pub extern "Rust" fn get(url: String) -> Result<String, String> {
    return match minreq::get(url.clone())
        .with_header("UserAgent", "smashnet.nro")
        .send() {
            Ok(response) => match response.as_str(){
                Ok(str) => Ok(format!("{}", str)),
                Err(e) => Err(format!("Error: {:?}", e))
            },
            Err(e) => Err(format!("Error: {:?}", e))
        }
}