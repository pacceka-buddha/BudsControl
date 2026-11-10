//! Operating-system appearance values the web UI and the tray icon need.

/// Reads a `REG_DWORD` from `HKEY_CURRENT_USER`.
#[cfg(windows)]
fn read_user_dword(subkey: windows::core::PCWSTR, value: windows::core::PCWSTR) -> Option<u32> {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};

    let mut data: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: `data` and `size` are valid for writes and `size` is the buffer's real size.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey,
            value,
            RRF_RT_REG_DWORD,
            None,
            Some(std::ptr::addr_of_mut!(data).cast()),
            Some(&mut size),
        )
    };
    status.is_ok().then_some(data)
}

/// The user's accent colour as `#RRGGBB`, if the OS exposes one.
#[cfg(windows)]
pub fn accent_color() -> Option<String> {
    use windows::core::w;
    read_user_dword(w!("Software\\Microsoft\\Windows\\DWM"), w!("AccentColor")).map(format_abgr)
}

#[cfg(not(windows))]
pub fn accent_color() -> Option<String> {
    None
}

/// Whether the taskbar (and so the tray) is light. The taskbar follows `SystemUsesLightTheme`, which
/// can differ from the apps' theme. `BUDS_TASKBAR_THEME=light|dark` overrides it, for testing the icon
/// without changing the system setting.
pub fn taskbar_is_light() -> bool {
    if let Ok(forced) = std::env::var("BUDS_TASKBAR_THEME") {
        match forced.to_ascii_lowercase().as_str() {
            "light" => return true,
            "dark" => return false,
            _ => {}
        }
    }
    system_uses_light_theme()
}

#[cfg(windows)]
fn system_uses_light_theme() -> bool {
    use windows::core::w;
    read_user_dword(
        w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
        w!("SystemUsesLightTheme"),
    )
    .is_some_and(|v| v != 0)
}

#[cfg(not(windows))]
fn system_uses_light_theme() -> bool {
    false
}

/// The DWM stores the colour as `0xAABBGGRR`.
#[cfg_attr(not(windows), allow(dead_code))]
fn format_abgr(value: u32) -> String {
    let (r, g, b) = (value & 0xFF, (value >> 8) & 0xFF, (value >> 16) & 0xFF);
    format!("#{r:02X}{g:02X}{b:02X}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abgr_to_hex() {
        assert_eq!(format_abgr(0xFFD77800), "#0078D7"); // Windows' classic blue
        assert_eq!(format_abgr(0xFF0000FF), "#FF0000");
    }

    #[test]
    fn the_environment_can_force_the_taskbar_theme() {
        // Tests in this binary run in parallel, so only this one touches the variable.
        // SAFETY: no other thread reads or writes the environment concurrently in this test binary.
        unsafe {
            std::env::set_var("BUDS_TASKBAR_THEME", "light");
            assert!(taskbar_is_light());
            std::env::set_var("BUDS_TASKBAR_THEME", "DARK");
            assert!(!taskbar_is_light());
            std::env::remove_var("BUDS_TASKBAR_THEME");
        }
    }
}
