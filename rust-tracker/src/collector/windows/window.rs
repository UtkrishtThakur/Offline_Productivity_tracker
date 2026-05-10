use serde_json::Value;

#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub app: String,
    pub title: String,
    pub workspace: i64,
}

#[cfg(target_os = "windows")]
pub fn get_active_window() -> Option<WindowInfo> {
    use windows::Win32::Foundation::{HWND, MAX_PATH};
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows::Win32::System::ProcessStatus::GetProcessImageFileNameW;

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0 == 0 {
            return None;
        }

        let mut title_buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut title_buf);
        let title = if len > 0 {
            String::from_utf16_lossy(&title_buf[..len as usize])
        } else {
            "unknown".to_string()
        };

        let mut process_id = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));

        let app = if let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) {
            let mut name_buf = [0u16; MAX_PATH as usize];
            let name_len = GetProcessImageFileNameW(process, &mut name_buf);
            if name_len > 0 {
                let full_path = String::from_utf16_lossy(&name_buf[..name_len as usize]);
                full_path.split('\\').last().unwrap_or("unknown").to_string()
            } else {
                "unknown".to_string()
            }
        } else {
            "unknown".to_string()
        };

        Some(WindowInfo {
            app,
            title,
            workspace: -1,
        })
    }
}

#[cfg(not(target_os = "windows"))]
pub fn get_active_window() -> Option<WindowInfo> {
    None
}
