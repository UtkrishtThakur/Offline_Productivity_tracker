#[cfg(target_os = "windows")]
pub fn get_idle_ms() -> Option<u64> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    use windows::Win32::System::SystemInformation::GetTickCount64;

    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };

        if GetLastInputInfo(&mut info).is_ok() {
            let tick_count = GetTickCount64();
            let idle_time = tick_count.saturating_sub(info.dwTime as u64);
            Some(idle_time)
        } else {
            None
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn get_idle_ms() -> Option<u64> {
    None
}
