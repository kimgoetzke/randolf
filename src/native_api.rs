use std::mem;
use winapi::shared::windef::{HWND, RECT};
use winapi::um::winuser::{
  GetForegroundWindow, GetMonitorInfoW, GetWindowPlacement, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
  SW_MAXIMIZE, SendMessageW, SetWindowPlacement, ShowWindow, WINDOWPLACEMENT, WM_PAINT,
};
use windows::Win32::Foundation::{LPARAM, WPARAM};

// TODO: Stop returning a window when no window is active
pub fn get_foreground_window() -> Option<HWND> {
  let window = unsafe { GetForegroundWindow() };
  if window.is_null() {
    debug!("There is no active window...");
    return None;
  }

  Some(window)
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
      warn!("Failed to get monitor info");
      return None;
    }
  }

  Some(monitor_info)
}

pub fn update_window_placement_and_force_repaint(window: HWND, placement: &WINDOWPLACEMENT) {
  unsafe {
    if SetWindowPlacement(window, placement) == 0 {
      warn!("Failed to set window placement for #{:?}", window);
    }

    // Force a repaint
    SendMessageW(window, WM_PAINT, 0, 0);
  }
}

pub fn maximise_window(window: HWND) {
  unsafe {
    ShowWindow(window, SW_MAXIMIZE);
  }
}

pub fn get_window_placement(window: HWND) -> Option<WINDOWPLACEMENT> {
  let mut placement: WINDOWPLACEMENT = unsafe { mem::zeroed() };
  placement.length = size_of::<WINDOWPLACEMENT>() as u32;

  unsafe {
    if GetWindowPlacement(window, &mut placement) == 0 {
      warn!("Failed to get window placement for window: {:?}", window);
      return None;
    }
  }

  Some(placement)
}

pub fn restore_window_placement(window: HWND, previous_placement: &WINDOWPLACEMENT) {
  unsafe {
    SetWindowPlacement(window, previous_placement);
    SendMessageW(window, WM_PAINT, 0, 0);
  }
}

pub fn close(window: HWND) {
  unsafe {
    use winapi::um::winuser::PostMessageW;
    use winapi::um::winuser::WM_CLOSE;

    PostMessageW(window, WM_CLOSE, 0, 0);
  }
}
