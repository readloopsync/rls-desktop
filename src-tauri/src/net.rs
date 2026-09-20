//! LAN address discovery for building the reader-facing service URLs.

/// The machine's primary LAN IPv4, e.g. "192.168.1.42". Falls back to
/// "127.0.0.1" if it can't be determined (the device won't reach that, but the
/// UI stays coherent and can tell the user).
pub fn lan_ip() -> String {
    match local_ip_address::local_ip() {
        Ok(std::net::IpAddr::V4(v4)) => v4.to_string(),
        _ => "127.0.0.1".to_string(),
    }
}
