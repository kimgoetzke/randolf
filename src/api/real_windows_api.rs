use crate::api::WindowsApi;
use crate::common::{Monitor, MonitorHandle, MonitorInfo, Monitors, Point, Rect, Window, WindowHandle, WindowPlacement};
use crate::configuration_provider::ExclusionSettings;
use std::mem::MaybeUninit;
use std::{mem, ptr};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
  EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO, MONITORINFOEXW,
  MonitorFromPoint, MonitorFromWindow,
};
use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, PROCESS_PER_MONITOR_DPI_AWARE, SetProcessDpiAwareness};
use windows::Win32::UI::Shell::IVirtualDesktopManager;
use windows::Win32::UI::WindowsAndMessaging::{
  DispatchMessageA, EnumWindows, GetClassNameW, GetCursorPos, GetForegroundWindow, GetWindowInfo, GetWindowPlacement,
  GetWindowTextW, IsIconic, IsWindowVisible, MSG, PM_REMOVE, PeekMessageA, PostMessageW, SW_HIDE, SW_MAXIMIZE, SW_MINIMIZE,
  SW_RESTORE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW, SendMessageW, SetCursorPos,
  SetForegroundWindow, SetWindowPlacement, SetWindowPos, ShowWindow, TranslateMessage, WINDOWINFO, WINDOWPLACEMENT,
  WM_CLOSE, WM_PAINT, WS_VISIBLE,
};
use windows::core::BOOL;

#[derive(Clone)]
pub struct RealWindowsApi {
  ignored_window_titles: Vec<String>,
  ignored_class_names: Vec<String>,
}

impl RealWindowsApi {
  pub fn new(settings: &ExclusionSettings) -> Self {
    Self {
      ignored_window_titles: settings.window_titles.clone(),
      ignored_class_names: settings.window_class_names.clone(),
    }
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
      if self.is_not_a_managed_window(&window.handle) || self.is_window_minimised(window.handle) {
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
    if self.ignored_class_names.contains(&class_name) {
      result = true;
    }

    let title = self.get_window_title(handle);
    if self.ignored_window_titles.contains(&title) {
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

  /// Sets the window position on the same monitor as the given rectangle. WARNING: Does not adjust for DPI scaling.
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

  // TODO: Try fixing the method below which aims to adjust the window position based on the DPI of the source and
  //   target monitors
  // This does not work yet and it turned out to be much easier to simply call SetWindowPos twice in a row which always
  // works because the second call will use the context of the target monitor.
  // Example of moving a near-maximised window from a 100% monitor to a 125% monitor:
  // Should be: Rect[(-1420, -367)-(-20, 2153), width: 1400, height: 2520]
  // But is:    Rect[(-1420, -367)-(-30, 2143), width: 1390, height: 2510] when using PROCESS_PER_MONITOR_DPI_AWARE and new code
  // Or is:     Rect[(-1420, -367)-(-308, 1641), width: 1112, height: 2008] when using DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 without new code
  fn set_window_position_with_dpi_adjustment(
    &self,
    window_handle: WindowHandle,
    source_monitor_handle: MonitorHandle,
    target_monitor_handle: MonitorHandle,
    rect: Rect,
  ) {
    unsafe {
      // let old_context = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
      let _ = SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE);

      let mut source_dpi_x = MaybeUninit::<u32>::uninit();
      let mut source_dpi_y = MaybeUninit::<u32>::uninit();
      if let Err(err) = GetDpiForMonitor(
        source_monitor_handle.as_h_monitor(),
        windows::Win32::UI::HiDpi::MDT_EFFECTIVE_DPI,
        source_dpi_x.as_mut_ptr(),
        source_dpi_y.as_mut_ptr(),
      ) {
        error!("Failed to get DPI for monitor {source_monitor_handle}: {}", err.message());
        return;
      }

      let source_dpi_x = source_dpi_x.assume_init();
      let source_dpi_y = source_dpi_y.assume_init();
      let source_scale_factor = source_dpi_x as f32 / 96.0;
      warn!(
        "DPI for source monitor {source_monitor_handle}: {source_scale_factor} with x={:?}dpi & y={:?}dpi",
        source_dpi_x, source_dpi_y
      );

      let mut target_dpi_x = MaybeUninit::<u32>::uninit();
      let mut target_dpi_y = MaybeUninit::<u32>::uninit();
      if let Err(err) = GetDpiForMonitor(
        target_monitor_handle.as_h_monitor(),
        windows::Win32::UI::HiDpi::MDT_EFFECTIVE_DPI,
        target_dpi_x.as_mut_ptr(),
        target_dpi_y.as_mut_ptr(),
      ) {
        error!("Failed to get DPI for monitor {target_monitor_handle}: {}", err.message());
        return;
      }

      let target_dpi_x = target_dpi_x.assume_init();
      let target_dpi_y = target_dpi_y.assume_init();
      let target_scale_factor = target_dpi_x as f32 / 96.0;
      warn!(
        "DPI for target monitor {target_monitor_handle}: {target_scale_factor} with x={:?}dpi & y={:?}dpi",
        target_dpi_x, target_dpi_y
      );

      let relative_scale = (target_scale_factor / source_scale_factor).clamp(0.1, 1.0);
      warn!(
        "Relative scale factor from source to target monitor: {relative_scale} (target: {target_scale_factor} / source: {source_scale_factor})"
      );

      let logical_left = rect.left;
      let logical_top = rect.top;
      let logical_width = ((rect.right - rect.left) as f32 / relative_scale).round() as i32 - 10;
      let logical_height = ((rect.bottom - rect.top) as f32 / relative_scale).round() as i32 - 10;

      warn!(
        "Adjusted to logical coordinates: {}, width={logical_width}, height={logical_height}",
        Point::new(logical_left, logical_top),
      );

      if let Err(err) = SetWindowPos(
        window_handle.as_hwnd(),
        Some(HWND(ptr::null_mut())),
        logical_left,
        logical_top,
        logical_width,
        logical_height,
        SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
      ) {
        warn!("Failed to set window position for window {window_handle}: {}", err.message());
      }

      // self.set_window_position(window_handle, rect);
      // SetThreadDpiAwarenessContext(old_context);
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

  fn do_minimise_window(&self, handle: WindowHandle) {
    unsafe {
      if !ShowWindow(handle.as_hwnd(), SW_MINIMIZE).as_bool() {
        warn!("Failed to minimise window {handle}");
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

  fn get_monitor_info_for_monitor(&self, handle: MonitorHandle) -> Option<MonitorInfo> {
    let mut monitor_info = empty_monitor_info();
    unsafe {
      let monitor = HMONITOR(handle.handle as *mut _);
      if !GetMonitorInfoW(monitor, &mut monitor_info).as_bool() {
        warn!("Failed to get monitor info for monitor that contains window {handle}");
        return None;
      }
    }

    Some(MonitorInfo::from(monitor_info))
  }

  fn get_monitor_id_for_handle(&self, handle: MonitorHandle) -> Option<[u16; 32]> {
    if let Some(monitor) = get_all_monitors().get_by_handle(handle) {
      Some(monitor.id)
    } else {
      error!("Failed to get monitor id for {handle}");

      None
    }
  }

  fn get_monitor_handle_for_window_handle(&self, handle: WindowHandle) -> MonitorHandle {
    unsafe { MonitorFromWindow(handle.as_hwnd(), MONITOR_DEFAULTTONEAREST) }.into()
  }

  fn get_monitor_handle_for_point(&self, point: &Point) -> MonitorHandle {
    unsafe { MonitorFromPoint(point.into(), MONITOR_DEFAULTTONEAREST).into() }
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
    trace!("- {}", monitor);
  }

  Monitors::from(monitors)
}

extern "system" fn enum_monitors_callback(hmonitor: HMONITOR, _dc: HDC, _rect: *mut RECT, data: LPARAM) -> BOOL {
  unsafe {
    let monitors = data.0 as *mut Vec<Monitor>;
    let mut device_info = MONITORINFOEXW::default();
    device_info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;

    if GetMonitorInfoW(hmonitor, &mut device_info as *mut MONITORINFOEXW as *mut MONITORINFO).as_bool() {
      let handle = MonitorHandle::from(hmonitor);
      let device_name = get_persistent_device_name(&handle, &device_info);
      (*monitors).push(Monitor::new(device_name, handle, device_info.monitorInfo));
    }

    true.into()
  }
}

fn get_persistent_device_name(_handle: &MonitorHandle, info: &MONITORINFOEXW) -> [u16; 32] {
  // trace!(
  //   "Persistent device name of {} is \"{}\"",
  //   handle,
  //   String::from_utf16_lossy(&info.szDevice)
  // );

  info.szDevice
}
