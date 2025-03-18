use winapi::shared::windef::{HWND, RECT};
use winapi::um::winuser::{GetForegroundWindow, GetMonitorInfoW, MonitorFromWindow, ShowWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST, SW_MAXIMIZE};

pub fn get_foreground_window() -> HWND {
  unsafe { GetForegroundWindow() }
}

pub fn get_monitor_info(window: HWND) -> Option<MONITORINFO> {
  let mut monitor_info = MONITORINFO {
    cbSize: size_of::<MONITORINFO>() as u32,
    rcMonitor: RECT {
      left: 0,
      top: 0,
      right: 0,
      bottom: 0,
    },
    rcWork: RECT {
      left: 0,
      top: 0,
      right: 0,
      bottom: 0,
    },
    dwFlags: 0,
  };

  unsafe {
    let monitor = MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST);
    if GetMonitorInfoW(monitor, &mut monitor_info) == 0 {
      error!("Failed to get monitor info");
      return None;
    }
  }

  Some(monitor_info)
}

pub fn maximise_window(window: HWND) {
  unsafe {
    ShowWindow(window, SW_MAXIMIZE);
  }
}