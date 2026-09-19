//! LAN address discovery, service URL construction, and QR-code SVG generation
//! for the popover UI.

use qrcode::render::svg;
use qrcode::{EcLevel, QrCode};

/// The machine's primary LAN IPv4, e.g. "192.168.1.42". Falls back to
/// "127.0.0.1" if it can't be determined (device won't reach that, but the UI
/// stays coherent and can tell the user).
pub fn lan_ip() -> String {
    match local_ip_address::local_ip() {
        Ok(std::net::IpAddr::V4(v4)) => v4.to_string(),
        _ => "127.0.0.1".to_string(),
    }
}

/// A compact SVG QR code for `data`, sized to `px`. Returned as an inline SVG
/// string the frontend drops straight into the DOM.
pub fn qr_svg(data: &str, px: u32) -> String {
    match QrCode::with_error_correction_level(data.as_bytes(), EcLevel::M) {
        Ok(code) => code
            .render::<svg::Color>()
            .min_dimensions(px, px)
            .max_dimensions(px, px)
            .quiet_zone(true)
            .dark_color(svg::Color("#111111"))
            .light_color(svg::Color("#ffffff"))
            .build(),
        Err(_) => String::new(),
    }
}
