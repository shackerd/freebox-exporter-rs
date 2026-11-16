pub const DEFAULT_FBX_HOST: &str = "mafreebox.freebox.fr";

/// API URL of the box, the same in router and bridge mode.
///
/// The box answers this name itself and intercepts its fixed address, so the API stays reachable
/// from the local network when the internet link is down. Its certificate covers this name, so
/// TLS is verified against the pinned Freebox CA.
pub fn api_url() -> String {
    format!("https://{DEFAULT_FBX_HOST}/api/")
}
