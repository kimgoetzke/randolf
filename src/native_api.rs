#![allow(dead_code)]
use crate::utils::{MonitorInfo, Point, Rect, Window, WindowHandle, WindowPlacement};
use std::mem::MaybeUninit;
use std::{mem, ptr};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
  EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
  MonitorFromWindow,
};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx};
use windows::Win32::UI::Shell::IVirtualDesktopManager;
use windows::Win32::UI::WindowsAndMessaging::{
  DispatchMessageA, EnumWindows, GetCursorPos, GetForegroundWindow, GetWindowInfo, GetWindowPlacement, GetWindowTextW, MSG,
  PM_REMOVE, PeekMessageA, PostMessageW, SW_MAXIMIZE, SendMessageW, SetCursorPos, SetForegroundWindow, SetWindowPlacement,
  ShowWindow, TranslateMessage, WINDOWINFO, WINDOWPLACEMENT, WM_CLOSE, WM_PAINT, WS_VISIBLE,
};
use windows::core::BOOL;

const IGNORED_WINDOWS: [&str; 4] = ["Program Manager", "Windows Input Experience", "Settings", ""];

pub fn get_foreground_window() -> Option<WindowHandle> {
  let hwnd = unsafe { GetForegroundWindow() };

  Some(hwnd.into())
}

pub fn get_monitor_info(handle: WindowHandle) -> Option<MonitorInfo> {
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
    let monitor = MonitorFromWindow(handle.as_hwnd(), MONITOR_DEFAULTTONEAREST);
    if GetMonitorInfoW(monitor, &mut monitor_info).0 == 0 {
      warn!("Failed to get monitor info for monitor that contains window {}", handle);
      return None;
    }
  }

  Some(MonitorInfo::from(monitor_info))
}

pub fn update_window_placement_and_force_repaint(handle: WindowHandle, placement: WindowPlacement) {
  let placement = placement.into();
  unsafe {
    if let Err(err) = SetWindowPlacement(handle.as_hwnd(), placement) {
      warn!("Failed to set window placement for {} because: {}", handle, err.message());
    }

    // Force a repaint
    SendMessageW(handle.as_hwnd(), WM_PAINT, Some(WPARAM(0)), Some(LPARAM(0)));
  }
}

pub fn maximise_window(handle: WindowHandle) {
  unsafe {
    if !ShowWindow(handle.as_hwnd(), SW_MAXIMIZE).as_bool() {
      warn!("Failed to maximise window {}", handle);
    }
  }
}

pub fn get_window_placement(handle: WindowHandle) -> Option<WindowPlacement> {
  let mut placement: WINDOWPLACEMENT = unsafe { mem::zeroed() };
  placement.length = size_of::<WINDOWPLACEMENT>() as u32;

  unsafe {
    if GetWindowPlacement(handle.as_hwnd(), &mut placement).is_err() {
      warn!("Failed to get window placement for window {}", handle);
      return None;
    }
  }

  Some(WindowPlacement::from(placement))
}

pub fn restore_window_placement(handle: WindowHandle, previous_placement: WindowPlacement) {
  unsafe {
    if let Err(err) = SetWindowPlacement(handle.as_hwnd(), previous_placement.into()) {
      warn!("Failed to restore window placement for {} because: {}", handle, err.message());
    }
    SendMessageW(handle.as_hwnd(), WM_PAINT, Some(WPARAM(0)), Some(LPARAM(0)));
  }
}

pub fn close(handle: WindowHandle) {
  unsafe {
    if PostMessageW(Option::from(handle.as_hwnd()), WM_CLOSE, WPARAM(0), LPARAM(0)).is_err() {
      warn!("Failed to close window {:?}", handle);
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

pub fn set_foreground_window(handle: WindowHandle) {
  unsafe {
    if !bool::from(SetForegroundWindow(handle.into())) {
      warn!("Failed to set foreground window to {}", handle);
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

pub fn get_all_visible_windows() -> Vec<Window> {
  let mut windows: Vec<Window> = Vec::new();
  unsafe {
    if let Err(err) = EnumWindows(Some(enum_window), LPARAM(&mut windows as *mut _ as isize)) {
      warn!("Failed to enumerate windows because: {}", err.message());
    }
  }

  trace!("┌| Found the following windows:");
  let mut i: usize = 1;
  windows.retain(|window| {
    if IGNORED_WINDOWS.contains(&window.title.as_str()) {
      false
    } else {
      let window_area = ((window.rect.right - window.rect.left) * (window.rect.bottom - window.rect.top)) / 1000;
      trace!(
        "├> {}. {:?} at ({}, {}) with a size of {}k sq px and title \"{}\"",
        i,
        window.handle,
        window.rect.left,
        window.rect.top,
        window_area,
        window.title_trunc()
      );
      i += 1;

      true
    }
  });
  trace!("└─| Identified [{:?}] windows", windows.len());

  windows
}

extern "system" fn enum_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
  unsafe {
    let windows = &mut *(lparam.0 as *mut Vec<Window>);
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
      let rect = Rect::from(info.rcWindow);
      let window_info = Window::new(title, rect, hwnd);
      windows.push(window_info);
    }

    true.into()
  }
}

fn get_window_info_safe(hwnd: HWND) -> Result<WINDOWINFO, &'static str> {
  unsafe {
    let mut info = WINDOWINFO {
      cbSize: size_of::<WINDOWINFO>() as u32,
      ..Default::default()
    };
    if GetWindowInfo(hwnd, &mut info).is_err() {
      return Err("Failed to get window info");
    }
    Ok(info)
  }
}

pub fn get_all_monitors() -> Vec<MONITORINFO> {
  let mut monitors: Vec<MONITORINFO> = Vec::new();
  unsafe {
    if EnumDisplayMonitors(
      None,
      Some(ptr::null_mut()),
      Some(enum_monitors),
      LPARAM(&mut monitors as *mut _ as isize),
    )
    .as_bool()
    {
      warn!("Failed to enumerate monitors");
    }
  }

  info!("Found [{}] monitors", monitors.len());
  let mut i: usize = 1;
  for monitor in &monitors {
    info!(
      "{i}. Monitor: size={}, monitor[{}, {}, {}, {}], work[{}, {}, {}, {}]",
      monitor.cbSize,
      monitor.rcMonitor.right,
      monitor.rcMonitor.left,
      monitor.rcMonitor.top,
      monitor.rcMonitor.bottom,
      monitor.rcWork.right,
      monitor.rcWork.left,
      monitor.rcWork.top,
      monitor.rcWork.bottom
    );
    i += 1;
  }

  monitors
}

extern "system" fn enum_monitors(handle: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
  unsafe {
    let monitors = &mut *(data.0 as *mut Vec<HMONITOR>);
    monitors.push(handle);
    true.into()
  }
}

pub fn get_monitor_for_window(window: isize) -> isize {
  unsafe { MonitorFromWindow(HWND(window as *mut core::ffi::c_void), MONITOR_DEFAULTTONEAREST) }.0 as isize
}

pub fn get_monitor_for_point(point: POINT) -> isize {
  unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST) }.0 as isize
}

pub fn get_virtual_desktop_manager() -> Option<IVirtualDesktopManager> {
  unsafe {
    let result = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    if result.is_err() {
      warn!("Failed to initialize COM because: {}", result.message());
      return None;
    }

    match CoCreateInstance(&windows::Win32::UI::Shell::VirtualDesktopManager, None, CLSCTX_ALL) {
      Ok(vdm) => Some(vdm),
      Err(err) => {
        warn!("Failed to get virtual desktop manager because: {}", err.message());
        None
      }
    }
  }
}

pub fn is_window_on_current_desktop(vdm: &IVirtualDesktopManager, window: &Window) -> Option<bool> {
  unsafe {
    match vdm.IsWindowOnCurrentVirtualDesktop(window.handle.into()) {
      Ok(is_on_current_desktop) => {
        let is_on_current_desktop = is_on_current_desktop.as_bool();
        trace!(
          "Skipping window {:?} \"{}\" - it is not on current desktop",
          window.handle,
          window.title_trunc()
        );
        Some(is_on_current_desktop)
      }
      Err(err) => {
        warn!("Failed to check if window is on current desktop because: {}", err.message());

        None
      }
    }
  }
}

pub fn process_windows_messages() {
  let mut msg = MaybeUninit::<MSG>::uninit();
  unsafe {
    if PeekMessageA(msg.as_mut_ptr(), Option::from(HWND(ptr::null_mut())), 0, 0, PM_REMOVE).into() {
      let _ = TranslateMessage(msg.as_ptr());
      DispatchMessageA(msg.as_ptr());
    }
  }
}
