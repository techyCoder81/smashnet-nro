
#[export_name = "Smashnet__get"]
pub extern "Rust" fn get(url: String) -> Result<String, String> {
    return match minreq::get(url.clone())
        .with_header("UserAgent", "smashnet.nro")
        .send() {
            Ok(response) => Ok(format!("{}", match response.as_str(){
                Ok(str) => str,
                Err(e) => format!("Error: {:?}", e).as_str()
            })),
            Err(e) => Err(format!("Error: {:?}", e))
        }
}