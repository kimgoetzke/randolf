use crate::point::Point;
use crate::rect::Rect;
use crate::window::{Window, WindowInfo};
use std::collections::HashMap;
use std::{mem, ptr};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
  EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
  MonitorFromWindow,
};
use windows::Win32::UI::WindowsAndMessaging::{
  EnumWindows, GetCursorPos, GetForegroundWindow, GetWindowInfo, GetWindowPlacement, GetWindowTextW, PostMessageW,
  SW_MAXIMIZE, SendMessageW, SetCursorPos, SetForegroundWindow, SetWindowPlacement, ShowWindow, WINDOWINFO, WINDOWPLACEMENT,
  WM_CLOSE, WM_PAINT, WS_VISIBLE,
};
use windows::core::BOOL;

const IGNORED_WINDOWS: [&str; 4] = ["Program Manager", "Windows Input Experience", "Settings", ""];

// TODO: Stop returning a window when no window is active
pub fn get_foreground_window() -> Option<HWND> {
  let hwnd = unsafe { GetForegroundWindow() };
  if hwnd == HWND(ptr::null_mut()) {
    debug!("There is no active window...");
    return None;
  }

  Some(hwnd)
}

pub fn get_monitor_info(hwnd: HWND) -> Option<MONITORINFO> {
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
    let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
    if GetMonitorInfoW(monitor, &mut monitor_info).0 == 0 {
      warn!("Failed to get monitor info");
      return None;
    }
  }

  Some(monitor_info)
}

pub fn update_window_placement_and_force_repaint(hwnd: HWND, placement: &WINDOWPLACEMENT) {
  unsafe {
    if let Err(err) = SetWindowPlacement(hwnd, placement) {
      warn!("Failed to set window placement for #{:?} because: {}", hwnd, err.message());
    }

    // Force a repaint
    SendMessageW(hwnd, WM_PAINT, Some(WPARAM(0)), Some(LPARAM(0)));
  }
}

pub fn maximise_window(hwnd: HWND) {
  unsafe {
    if !bool::from(ShowWindow(hwnd, SW_MAXIMIZE)) {
      warn!("Failed to maximise window #{:?}", hwnd);
    }
  }
}

pub fn get_window_placement(window: HWND) -> Option<WINDOWPLACEMENT> {
  let mut placement: WINDOWPLACEMENT = unsafe { mem::zeroed() };
  placement.length = size_of::<WINDOWPLACEMENT>() as u32;

  unsafe {
    if GetWindowPlacement(window, &mut placement).is_err() {
      warn!("Failed to get window placement for window: {:?}", window);
      return None;
    }
  }

  Some(placement)
}

pub fn restore_window_placement(window: HWND, previous_placement: &WINDOWPLACEMENT) {
  unsafe {
    if let Err(err) = SetWindowPlacement(window, previous_placement) {
      warn!(
        "Failed to restore window placement for #{:?} because: {}",
        window,
        err.message()
      );
    }
    SendMessageW(window, WM_PAINT, Some(WPARAM(0)), Some(LPARAM(0)));
  }
}

pub fn close(window: HWND) {
  unsafe {
    if PostMessageW(Option::from(window), WM_CLOSE, WPARAM(0), LPARAM(0)).is_err() {
      warn!("Failed to close window: {:?}", window);
    }
  }
}

pub fn get_cursor_position() -> Point {
  let mut point: POINT = unsafe { mem::zeroed() };
  unsafe {
    if let Err(err) = GetCursorPos(&mut point) {
      warn!("Failed to get cursor position because: {}", err.message());
    }
  }

  Point::new(point.x, point.y)
}

pub fn set_foreground_window(window: Window) {
  unsafe {
    if !bool::from(SetForegroundWindow(window.into())) {
      warn!("Failed to set foreground window to {}", window);
    }
  }
}

pub fn set_cursor_position(target_point: &Point) {
  unsafe {
    if let Err(err) = SetCursorPos(target_point.x(), target_point.y()) {
      warn!("Failed to set cursor position to {} because: {}", target_point, err.message());
    }
  }
}

pub fn get_all_visible_windows() -> HashMap<Window, WindowInfo> {
  let mut windows: HashMap<Window, WindowInfo> = HashMap::new();
  unsafe {
    if let Err(err) = EnumWindows(Some(enum_window), LPARAM(&mut windows as *mut _ as isize)) {
      warn!("Failed to enumerate windows because: {}", err.message());
    }
  }

  debug!("┌| Found the following windows:");
  let mut i: usize = 1;
  windows.retain(|_, window_info| {
    if IGNORED_WINDOWS.contains(&window_info.title.as_str()) {
      false
    } else {
      let window_area =
        ((window_info.rect.right - window_info.rect.left) * (window_info.rect.bottom - window_info.rect.top)) / 1000;
      debug!(
        "├> {}. #{:?} at ({}, {}) with a size of {}k sq px and title \"{}\"",
        i, window_info.hwnd, window_info.rect.left, window_info.rect.top, window_area, window_info.title
      );
      i += 1;

      true
    }
  });
  debug!("└─| Identified [{:?}] windows", windows.len());

  windows
}

extern "system" fn enum_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
  unsafe {
    let windows = &mut *(lparam.0 as *mut HashMap<Window, WindowInfo>);
    if hwnd.0.is_null() {
      return true.into();
    }

    let info = match get_window_info_safe(hwnd) {
      Ok(info) => info,
      Err(_) => return true.into(),
    };
    if !info.dwStyle.contains(WS_VISIBLE) {
      return true.into();
    }

    let mut text: [u16; 512] = [0; 512];
    let len = GetWindowTextW(hwnd, &mut text);
    let title = String::from_utf16_lossy(&text[..len as usize]);
    if !title.is_empty() {
      let window = Window::from(hwnd);
      let rect = Rect::from(info.rcWindow);
      let window_info = WindowInfo::new(title, rect, hwnd);
      windows.insert(window, window_info);
    }

    true.into()
  }
}

fn get_window_info_safe(window: HWND) -> Result<WINDOWINFO, &'static str> {
  unsafe {
    let mut info = WINDOWINFO {
      cbSize: size_of::<WINDOWINFO>() as u32,
      ..Default::default()
    };
    if GetWindowInfo(window, &mut info).is_err() {
      return Err("Failed to get window info");
    }
    Ok(info)
  }
}

pub fn get_window_title(window: HWND) -> String {
  unsafe {
    let mut text: [u16; 512] = [0; 512];
    let len = GetWindowTextW(window, &mut text);

    String::from_utf16_lossy(&text[..len as usize])
  }
}

pub fn get_all_monitors() -> Vec<MONITORINFO> {
  let mut monitors: Vec<MONITORINFO> = Vec::new();
  unsafe {
    if !bool::from(EnumDisplayMonitors(
      None,
      Some(ptr::null_mut()),
      Some(enum_monitors),
      LPARAM(&mut monitors as *mut _ as isize),
    )) {
      warn!("Failed to enumerate monitors");
    }
  }

  info!("Found [{}] monitors", monitors.len());
  monitors
}

unsafe extern "system" fn enum_monitors(handle: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
  let monitors = &mut *(data.0 as *mut Vec<HMONITOR>);
  monitors.push(handle);
  true.into()
}

pub fn get_monitor_for_window(window: isize) -> isize {
  unsafe { MonitorFromWindow(HWND(window as *mut core::ffi::c_void), MONITOR_DEFAULTTONEAREST) }.0 as isize
}

pub fn get_monitor_for_point(point: POINT) -> isize {
  unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST) }.0 as isize
}
