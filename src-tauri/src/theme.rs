//! Operating-system appearance values the web UI can't read itself.

/// The user's accent colour as `#RRGGBB`, if the OS exposes one.
#[cfg(windows)]
pub fn accent_color() -> Option<String> {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    use windows::core::w;

    let mut value: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: `value` and `size` are valid for writes and `size` is the buffer's real size.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\DWM"),
            w!("AccentColor"),
            RRF_RT_REG_DWORD,
            None,
            Some(std::ptr::addr_of_mut!(value).cast()),
            Some(&mut size),
        )
    };
    status.is_ok().then(|| format_abgr(value))
}

#[cfg(not(windows))]
pub fn accent_color() -> Option<String> {
    None
}

/// The DWM stores the colour as `0xAABBGGRR`.
#[cfg_attr(not(windows), allow(dead_code))]
fn format_abgr(value: u32) -> String {
    let (r, g, b) = (value & 0xFF, (value >> 8) & 0xFF, (value >> 16) & 0xFF);
    format!("#{r:02X}{g:02X}{b:02X}")
}

#[cfg(test)]
mod tests {
    use super::format_abgr;

    #[test]
    fn abgr_to_hex() {
        assert_eq!(format_abgr(0xFFD77800), "#0078D7"); // Windows' classic blue
        assert_eq!(format_abgr(0xFF0000FF), "#FF0000");
    }
}
