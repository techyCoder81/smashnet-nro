
#[export_name = "Smashnet__get"]
pub extern "Rust" fn get(url: String) -> Result<String, String> {
    return match minreq::get(url.clone())
        .with_header("UserAgent", "smashnet.nro")
        .send() {
            Ok(response) => Ok(response.to_owned()),
            Err(e) => Err(format!("Error: {}", e))
        }
}