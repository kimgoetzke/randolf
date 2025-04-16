use crate::api::WindowsApi;
use crate::utils::WindowHandle;
use crate::utils::{Monitor, MonitorInfo, Monitors, Point, Rect, Window, WindowPlacement};
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
  DispatchMessageA, EnumWindows, GetClassNameW, GetCursorPos, GetForegroundWindow, GetWindowInfo, GetWindowPlacement,
  GetWindowTextW, IsIconic, IsWindowVisible, MSG, PM_REMOVE, PeekMessageA, PostMessageW, SW_HIDE, SW_MAXIMIZE, SW_RESTORE,
  SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW, SendMessageW, SetCursorPos, SetForegroundWindow,
  SetWindowPlacement, SetWindowPos, ShowWindow, TranslateMessage, WINDOWINFO, WINDOWPLACEMENT, WM_CLOSE, WM_PAINT,
  WS_VISIBLE,
};
use windows::core::BOOL;

const IGNORED_WINDOW_TITLES: [&str; 4] = ["Program Manager", "Windows Input Experience", "Settings", ""];
const IGNORED_CLASS_NAMES: [&str; 5] = [
  "Progman",
  "WorkerW",
  "Shell_TrayWnd",
  "Shell_SecondaryTrayWnd",
  "DV2ControlHost",
];

#[derive(Copy, Clone)]
pub struct RealWindowsApi;

impl RealWindowsApi {
  pub fn new() -> Self {
    Self
  }
}

impl WindowsApi for RealWindowsApi {
  fn get_foreground_window(&self) -> Option<WindowHandle> {
    let hwnd = unsafe { GetForegroundWindow() };

    let handle = WindowHandle::from(hwnd);
    if self.is_not_a_managed_window(&handle) {
      return None;
    }

    Some(handle)
  }

  fn set_foreground_window(&self, handle: WindowHandle) {
    unsafe {
      if !SetForegroundWindow(handle.into()).as_bool() {
        warn!("Failed to set foreground window to {handle}");
      }
    }
  }

  fn get_all_visible_windows(&self) -> Vec<Window> {
    let mut windows = get_all_windows();

    trace!("┌| Found the following windows:");
    let mut i: usize = 1;
    windows.retain(|window| {
      if self.is_not_a_managed_window(&window.handle) {
        false
      } else {
        let window_area = ((window.rect.right - window.rect.left) * (window.rect.bottom - window.rect.top)) / 1000;
        trace!(
          "├> {}. {} at ({}, {}) with a size of {window_area}k sq px and title \"{}\"",
          i,
          window.handle,
          window.rect.left,
          window.rect.top,
          window.title_trunc()
        );
        i += 1;

        true
      }
    });
    trace!("└─| Identified [{:?}] windows", windows.len());

    windows
  }

  fn get_all_visible_windows_within_area(&self, rect: Rect) -> Vec<Window> {
    let mut windows = get_all_windows();

    windows.retain(|window| {
      if self.is_not_a_managed_window(&window.handle) {
        false
      } else {
        !(window.rect.left < rect.left
          || window.rect.right > rect.right
          || window.rect.top < rect.top
          || window.rect.bottom > rect.bottom)
      }
    });

    windows
  }

  fn get_window_title(&self, handle: &WindowHandle) -> String {
    let mut text: [u16; 512] = [0; 512];
    let len = unsafe { GetWindowTextW(handle.as_hwnd(), &mut text) };
    String::from_utf16_lossy(&text[..len as usize])
  }

  fn get_window_class_name(&self, handle: &WindowHandle) -> String {
    let mut class_name: [u16; 256] = [0; 256];
    let len = unsafe { GetClassNameW(handle.as_hwnd(), &mut class_name) };
    String::from_utf16_lossy(&class_name[..len as usize])
  }

  fn is_window_minimised(&self, handle: WindowHandle) -> bool {
    unsafe { IsIconic(handle.as_hwnd()).as_bool() }
  }

  fn is_not_a_managed_window(&self, handle: &WindowHandle) -> bool {
    let mut result = false;
    let class_name = self.get_window_class_name(handle);
    if IGNORED_CLASS_NAMES.contains(&class_name.as_str()) {
      result = true;
    }

    let title = self.get_window_title(handle);
    if IGNORED_WINDOW_TITLES.contains(&title.as_str()) {
      result = true;
    }

    // debug!(
    //   "{}  {} {} being managed (class name [{}] and title [\"{}\"])",
    //   if result { "⛔" } else { "✅" },
    //   handle,
    //   if result { "is NOT" } else { "is" },
    //   class_name,
    //   title,
    // );
    result
  }

  fn is_window_hidden(&self, handle: &WindowHandle) -> bool {
    unsafe { !IsWindowVisible(handle.as_hwnd()).as_bool() }
  }

  fn set_window_position(&self, handle: WindowHandle, rect: Rect) {
    unsafe {
      if let Err(err) = SetWindowPos(
        handle.as_hwnd(),
        Some(HWND(ptr::null_mut())),
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
        SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
      ) {
        warn!("Failed to set window position for window {handle}: {}", err.message());
      }
    }
  }

  fn do_restore_window(&self, window: &Window, is_minimised: &bool) {
    debug!("Restoring window {}", window.handle);
    unsafe {
      if !*is_minimised {
        let _ = !ShowWindow(window.handle.as_hwnd(), SW_RESTORE);
      }
      if let Err(err) = SetWindowPos(
        window.handle.as_hwnd(),
        None,
        window.rect.left,
        window.rect.top,
        window.rect.right - window.rect.left,
        window.rect.bottom - window.rect.top,
        SWP_SHOWWINDOW,
      ) {
        warn!(
          "Failed to set window position for window {}: {}",
          window.handle,
          err.message()
        );
      }
    }
  }

  fn do_maximise_window(&self, handle: WindowHandle) {
    unsafe {
      if !ShowWindow(handle.as_hwnd(), SW_MAXIMIZE).as_bool() {
        warn!("Failed to maximise window {handle}");
      }
    }
  }

  fn do_hide_window(&self, handle: WindowHandle) {
    unsafe {
      if !ShowWindow(handle.as_hwnd(), SW_HIDE).as_bool() {
        warn!("Failed to hide window {handle}");
      }
    }
  }

  fn do_close_window(&self, handle: WindowHandle) {
    unsafe {
      if let Err(err) = PostMessageW(Option::from(handle.as_hwnd()), WM_CLOSE, WPARAM(0), LPARAM(0)) {
        warn!("Failed to close window {:?} because: {}", handle, err.message());
      }
    }
  }

  fn get_window_placement(&self, handle: WindowHandle) -> Option<WindowPlacement> {
    let mut placement: WINDOWPLACEMENT = unsafe { mem::zeroed() };
    placement.length = size_of::<WINDOWPLACEMENT>() as u32;

    unsafe {
      if GetWindowPlacement(handle.as_hwnd(), &mut placement).is_err() {
        warn!("Failed to get window placement for window {handle}");
        return None;
      }
    }

    Some(WindowPlacement::from(placement))
  }

  fn set_window_placement_and_force_repaint(&self, handle: WindowHandle, placement: WindowPlacement) {
    let placement = placement.into();
    unsafe {
      if let Err(err) = SetWindowPlacement(handle.as_hwnd(), placement) {
        warn!("Failed to set window placement for {handle} because: {}", err.message());
      }

      // Force a repaint
      SendMessageW(handle.as_hwnd(), WM_PAINT, Some(WPARAM(0)), Some(LPARAM(0)));
    }
  }

  fn do_restore_window_placement(&self, handle: WindowHandle, previous_placement: WindowPlacement) {
    unsafe {
      if let Err(err) = SetWindowPlacement(handle.as_hwnd(), previous_placement.into()) {
        warn!("Failed to restore window placement for {handle} because: {}", err.message());
      }
      SendMessageW(handle.as_hwnd(), WM_PAINT, Some(WPARAM(0)), Some(LPARAM(0)));
    }
  }

  fn get_cursor_position(&self) -> Point {
    let mut point: POINT = unsafe { mem::zeroed() };
    unsafe {
      if let Err(err) = GetCursorPos(&mut point) {
        warn!("Failed to get cursor position because: {}", err.message());
      }
    }

    Point::new(point.x, point.y)
  }

  fn set_cursor_position(&self, target_point: &Point) {
    unsafe {
      if let Err(err) = SetCursorPos(target_point.x(), target_point.y()) {
        warn!("Failed to set cursor position to {target_point} because: {}", err.message());
      }
    }
  }

  fn get_all_monitors(&self) -> Monitors {
    get_all_monitors()
  }

  fn get_monitor_info_for_window(&self, handle: WindowHandle) -> Option<MonitorInfo> {
    let mut monitor_info = empty_monitor_info();
    unsafe {
      let monitor = MonitorFromWindow(handle.as_hwnd(), MONITOR_DEFAULTTONEAREST);
      if !GetMonitorInfoW(monitor, &mut monitor_info).as_bool() {
        warn!("Failed to get monitor info for monitor that contains window {handle}");
        return None;
      }
    }

    Some(MonitorInfo::from(monitor_info))
  }

  // TODO: Use dedicated MonitorHandle struct and stop using isize everywhere
  fn get_monitor_info_for_monitor(&self, handle: isize) -> Option<MonitorInfo> {
    let mut monitor_info = empty_monitor_info();
    unsafe {
      let monitor = HMONITOR(handle as *mut _);
      if !GetMonitorInfoW(monitor, &mut monitor_info).as_bool() {
        warn!("Failed to get monitor info for monitor that contains window {handle}");
        return None;
      }
    }

    Some(MonitorInfo::from(monitor_info))
  }

  fn get_monitor_for_window_handle(&self, handle: WindowHandle) -> isize {
    unsafe { MonitorFromWindow(handle.as_hwnd(), MONITOR_DEFAULTTONEAREST) }.0 as isize
  }

  fn get_monitor_for_point(&self, point: &Point) -> isize {
    unsafe { MonitorFromPoint(point.into(), MONITOR_DEFAULTTONEAREST) }.0 as isize
  }

  fn get_virtual_desktop_manager(&self) -> Option<IVirtualDesktopManager> {
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

  fn is_window_on_current_desktop(&self, vdm: &IVirtualDesktopManager, window: &Window) -> Option<bool> {
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
}

extern "system" fn enum_visible_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
  unsafe {
    let windows = &mut *(lparam.0 as *mut Vec<Window>);
    if hwnd.0.is_null() {
      return true.into();
    }

    let info = match get_window_info(hwnd) {
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
      let window = Window::new(hwnd, title, rect);
      windows.push(window);
    }

    true.into()
  }
}

fn get_all_windows() -> Vec<Window> {
  let mut windows: Vec<Window> = Vec::new();
  unsafe {
    if let Err(err) = EnumWindows(Some(enum_visible_windows_callback), LPARAM(&mut windows as *mut _ as isize)) {
      warn!("Failed to enumerate windows because: {}", err.message());
    }
  }

  windows
}

fn get_window_info(hwnd: HWND) -> Result<WINDOWINFO, &'static str> {
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

pub fn do_process_windows_messages() {
  let mut msg = MaybeUninit::<MSG>::uninit();
  unsafe {
    if PeekMessageA(msg.as_mut_ptr(), Option::from(HWND(ptr::null_mut())), 0, 0, PM_REMOVE).into() {
      let _ = TranslateMessage(msg.as_ptr());
      DispatchMessageA(msg.as_ptr());
    }
  }
}

fn empty_monitor_info() -> MONITORINFO {
  MONITORINFO {
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
  }
}

pub fn get_all_monitors() -> Monitors {
  let mut monitors: Vec<Monitor> = Vec::new();

  unsafe {
    if !EnumDisplayMonitors(
      None,
      Some(ptr::null_mut()),
      Some(enum_monitors_callback),
      LPARAM(&mut monitors as *mut Vec<Monitor> as isize),
    )
    .as_bool()
    {
      warn!("Failed to enumerate monitors");
    }
  }

  for monitor in &monitors {
    trace!("- {}", monitor,);
  }

  Monitors::from(monitors)
}

extern "system" fn enum_monitors_callback(monitor: HMONITOR, _dc: HDC, _rect: *mut RECT, data: LPARAM) -> BOOL {
  unsafe {
    let monitors = data.0 as *mut Vec<Monitor>;
    let mut monitor_info = MONITORINFO {
      cbSize: size_of::<MONITORINFO>() as u32,
      ..Default::default()
    };

    if GetMonitorInfoW(monitor, &mut monitor_info).as_bool() {
      (*monitors).push(Monitor::new(monitor, monitor_info));
    }

    true.into()
  }
}
