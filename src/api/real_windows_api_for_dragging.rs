use crate::common::{Command, DragState, Point, Rect, ResizeMode, ResizeState, WindowHandle};
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

static IS_WIN_KEY_PRESSED: AtomicBool = AtomicBool::new(false);
static IS_DRAGGING: AtomicBool = AtomicBool::new(false);
static IS_RESIZING: AtomicBool = AtomicBool::new(false);
static DRAG_STATE: OnceLock<Arc<Mutex<DragState>>> = OnceLock::new();
static RESIZE_STATE: OnceLock<Arc<Mutex<ResizeState>>> = OnceLock::new();
static MOUSE_HOOK_HANDLE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
static HOOK_TIMER_ID: AtomicUsize = AtomicUsize::new(0);
static SENDER: OnceLock<Arc<Mutex<Sender<Command>>>> = OnceLock::new();
static KEY_PRESS_DELAY_IN_MS: OnceLock<u32> = OnceLock::new();

pub struct WindowsApiForDragging {
  keyboard_hook_handle: Option<HHOOK>,
}

impl WindowsApiForDragging {
  pub fn new(sender: Sender<Command>, key_press_delay_in_ms: u32) -> Self {
    SENDER
      .set(Arc::new(Mutex::new(sender)))
      .expect("Failed to set command sender");
    KEY_PRESS_DELAY_IN_MS
      .set(key_press_delay_in_ms)
      .expect("Failed to set key press delay in");
    Self {
      keyboard_hook_handle: None,
    }
  }

  pub fn initialise(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
      let h_module = GetModuleHandleW(None)?;
      let h_instance = HINSTANCE(h_module.0);
      let keyboard_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(Self::keyboard_callback), Option::from(h_instance), 0)?;

      self.keyboard_hook_handle = Some(keyboard_hook);
    }

    Ok(())
  }

  // TODO: Fix bug where start menu opens after operation
  extern "system" fn keyboard_callback(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    unsafe {
      if n_code == HC_ACTION as i32 {
        let keyboard_data = *(l_param.0 as *const KBDLLHOOKSTRUCT);
        let vk_code = keyboard_data.vkCode;
        let is_window_key = vk_code == VK_LWIN.0 as u32 || vk_code == VK_RWIN.0 as u32;
        if is_window_key {
          let is_pressed = (w_param.0 as u32) == WM_KEYDOWN || (w_param.0 as u32) == WM_SYSKEYDOWN;
          if is_pressed == IS_WIN_KEY_PRESSED.load(Ordering::Relaxed) {
            return CallNextHookEx(None, n_code, w_param, l_param);
          }

          trace!("Win key [{}] {}", vk_code, if is_pressed { "pressed" } else { "released" });
          IS_WIN_KEY_PRESSED.store(is_pressed, Ordering::Relaxed);
          if is_pressed {
            Self::start_mouse_hook_install_timer();
          } else if HOOK_TIMER_ID.load(Ordering::Relaxed) == 0 {
            Self::cancel_mouse_hook_install_timer();
            if IS_DRAGGING.load(Ordering::Relaxed) {
              Self::finish_dragging();
            }
            if IS_RESIZING.load(Ordering::Relaxed) {
              Self::finish_resizing();
            }
            SENDER
              .get()
              .expect("Command sender not initialised")
              .lock()
              .expect("Failed to acquire command sender lock")
              .send(Command::DragWindows(false))
              .expect("Failed to send drag window command");
            Self::uninstall_mouse_hook();
          }
        }
      }

      CallNextHookEx(None, n_code, w_param, l_param)
    }
  }

  fn start_mouse_hook_install_timer() {
    unsafe {
      Self::cancel_mouse_hook_install_timer();
      let key_press_delay_in_ms = KEY_PRESS_DELAY_IN_MS.get().expect("Key press delay not initialised");
      let timer_id = SetTimer(None, 1000, *key_press_delay_in_ms, Some(Self::timer_callback));
      if timer_id != 0 {
        HOOK_TIMER_ID.store(timer_id, Ordering::Relaxed);
        trace!("Started hook installation timer with ID {}", timer_id);
      } else {
        error!("Failed to create hook installation timer");
      }
    }
  }

  extern "system" fn timer_callback(_hwnd: HWND, _msg: u32, timer_id: usize, _time: u32) {
    if HOOK_TIMER_ID.load(Ordering::Relaxed) == timer_id {
      Self::cancel_mouse_hook_install_timer();
      if IS_WIN_KEY_PRESSED.load(Ordering::Relaxed) {
        Self::install_mouse_hook();
        SENDER
          .get()
          .expect("Command sender not initialised")
          .lock()
          .expect("Failed to acquire command sender lock")
          .send(Command::DragWindows(true))
          .expect("Failed to send drag window command");
        let key_press_delay_in_ms = KEY_PRESS_DELAY_IN_MS.get().expect("Key press delay not initialised");
        debug!("Installed mouse hook after {}ms delay", key_press_delay_in_ms);
      } else {
        trace!("Win key no longer pressed when timer expired");
      }
    }
  }

  fn cancel_mouse_hook_install_timer() {
    unsafe {
      let timer_id = HOOK_TIMER_ID.swap(0, Ordering::Relaxed);
      if timer_id != 0 {
        if let Err(err) = KillTimer(None, timer_id) {
          error!("Failed to cancel hook installation timer: {}", err);
        } else {
          trace!("Cancelled hook installation timer with ID {}", timer_id);
        }
      }
    }
  }

  fn install_mouse_hook() {
    unsafe {
      if !MOUSE_HOOK_HANDLE.load(Ordering::Relaxed).is_null() {
        return;
      }
      let h_module = GetModuleHandleW(None).expect("Failed to get module handle");
      let h_instance = HINSTANCE(h_module.0);
      if let Ok(mouse_hook) =
        SetWindowsHookExW(WH_MOUSE_LL, Some(Self::low_level_mouse_callback), Option::from(h_instance), 0)
      {
        MOUSE_HOOK_HANDLE.store(mouse_hook.0, Ordering::Relaxed);
        trace!("Mouse hook installed");
      } else {
        error!("Failed to install mouse hook");
      }
    }
  }

  /// Uninstalls the mouse hook if it is currently installed. Does nothing if the hook is not installed.
  fn uninstall_mouse_hook() {
    unsafe {
      let hook_pointer = MOUSE_HOOK_HANDLE.swap(std::ptr::null_mut(), Ordering::Relaxed);
      if !hook_pointer.is_null() {
        let hook = HHOOK(hook_pointer);
        if let Err(err) = UnhookWindowsHookEx(hook) {
          error!("Failed to unhook mouse hook: {}", err);
        } else {
          trace!("Mouse hook uninstalled");
        }
      }
    }
  }

  extern "system" fn low_level_mouse_callback(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    unsafe {
      if n_code != HC_ACTION as i32 {
        return CallNextHookEx(None, n_code, w_param, l_param);
      }

      if !IS_WIN_KEY_PRESSED.load(Ordering::Relaxed) {
        return CallNextHookEx(None, n_code, w_param, l_param);
      }

      match w_param.0 as u32 {
        WM_LBUTTONDOWN => {
          let mouse_low_level_hook_struct = *(l_param.0 as *const MSLLHOOKSTRUCT);
          let cursor_position = Point::from(mouse_low_level_hook_struct.pt);
          debug!("Win key + left mouse button pressed at {}, starting drag...", cursor_position);
          Self::start_dragging(cursor_position);
          return LRESULT(1);
        }
        WM_LBUTTONUP => {
          if IS_DRAGGING.load(Ordering::Relaxed) {
            debug!("Win key + left mouse button released, ending drag...",);
            Self::finish_dragging();
            return LRESULT(1);
          }
        }
        WM_RBUTTONDOWN => {
          let mouse_low_level_hook_struct = *(l_param.0 as *const MSLLHOOKSTRUCT);
          let cursor_position = Point::from(mouse_low_level_hook_struct.pt);
          debug!(
            "Win key + right mouse button pressed at {}, starting resize...",
            cursor_position
          );
          Self::start_resizing(cursor_position);
          return LRESULT(1);
        }
        WM_RBUTTONUP => {
          if IS_RESIZING.load(Ordering::Relaxed) {
            debug!("Win key + right mouse button released, ending window resizing...");
            Self::finish_resizing();
            return LRESULT(1);
          }
        }
        WM_MOUSEMOVE => {
          if IS_DRAGGING.load(Ordering::Relaxed) {
            let mouse_low_level_hook_struct = *(l_param.0 as *const MSLLHOOKSTRUCT);
            Self::do_drag(mouse_low_level_hook_struct.pt);
            return CallNextHookEx(None, n_code, w_param, l_param);
          } else if IS_RESIZING.load(Ordering::Relaxed) {
            let mouse_low_level_hook_struct = *(l_param.0 as *const MSLLHOOKSTRUCT);
            Self::do_resize(mouse_low_level_hook_struct.pt);
            return CallNextHookEx(None, n_code, w_param, l_param);
          }
        }
        _ => return CallNextHookEx(None, n_code, w_param, l_param),
      }

      CallNextHookEx(None, n_code, w_param, l_param)
    }
  }

  fn start_dragging(cursor_position: Point) {
    unsafe {
      let hwnd_under_cursor = WindowFromPoint(cursor_position.as_point());
      if hwnd_under_cursor.0.is_null() {
        debug!("No window under cursor at {}", cursor_position);
        return;
      }
      let target_hwnd = Self::get_top_level_hwnd(hwnd_under_cursor);
      if target_hwnd.0.is_null() {
        debug!("No top-level HWND found under cursor at {}", cursor_position);
        return;
      }
      if !Self::can_move_window(target_hwnd) {
        debug!("Cannot move window with HWND: {:?}", target_hwnd);
        return;
      }
      let mut window_rect = RECT::default();
      if GetWindowRect(target_hwnd, &mut window_rect).is_err() {
        error!("Failed to get window rect for HWND: {:?}", target_hwnd);
        return;
      }
      if !SetForegroundWindow(target_hwnd).as_bool() {
        warn!("Failed to set foreground window to w#{:?}", target_hwnd.0);
      }
      if let Ok(mut drag_state) = get_drag_state().lock() {
        let window_position = Point::new(window_rect.left, window_rect.top);
        let window_handle = WindowHandle::from(target_hwnd);
        drag_state.set(cursor_position, window_handle, window_position);
        IS_DRAGGING.store(true, Ordering::Relaxed);
      }
    }
  }

  fn do_drag(cursor_point: POINT) {
    let drag_state = get_drag_state();
    let drag_guard = match drag_state.lock() {
      Ok(guard) => guard,
      Err(_) => return,
    };
    if !IS_DRAGGING.load(Ordering::Relaxed) {
      debug!(
        "Not dragging, ignoring mouse move at ({}, {})",
        cursor_point.x, cursor_point.y
      );
      return;
    }
    let drag_start_position = drag_guard.get_drag_start_position();
    let window_start_position = drag_guard.get_window_start_position();
    let delta_x = cursor_point.x - drag_start_position.x();
    let delta_y = cursor_point.y - drag_start_position.y();
    let new_x = window_start_position.x() + delta_x;
    let new_y = window_start_position.y() + delta_y;
    let window_hwnd = match drag_guard.get_window_handle() {
      Some(handle) => handle.as_hwnd(),
      None => {
        error!("Failed to retrieve the window handle for dragging, ignoring operation...");
        return;
      }
    };
    drop(drag_guard);

    trace!("Dragging window to ({}, {})", new_x, new_y);
    unsafe {
      if let Err(err) = SetWindowPos(
        window_hwnd,
        None,
        new_x,
        new_y,
        0,
        0,
        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
      ) {
        error!("Failed to set window position: {}", err);
      }
    }
  }

  fn finish_dragging() {
    if let Ok(mut drag_state) = get_drag_state().lock() {
      drag_state.reset();
      IS_DRAGGING.store(false, Ordering::Relaxed);
    }
  }

  fn can_move_window(window: HWND) -> bool {
    let placement = WINDOWPLACEMENT {
      length: size_of::<WINDOWPLACEMENT>() as u32,
      ..Default::default()
    };

    unsafe {
      if GetWindowPlacement(window, &placement as *const _ as *mut _).is_ok() {
        placement.showCmd != SW_SHOWMINIMIZED.0 as u32 && placement.showCmd != SW_SHOWMAXIMIZED.0 as u32
      } else {
        false
      }
    }
  }

  fn start_resizing(cursor_position: Point) {
    unsafe {
      let hwnd_under_cursor = WindowFromPoint(cursor_position.as_point());
      if hwnd_under_cursor.0.is_null() {
        debug!("No window under cursor at {}", cursor_position);
        return;
      }
      let target_hwnd = Self::get_top_level_hwnd(hwnd_under_cursor);
      if target_hwnd.0.is_null() {
        debug!("No top-level HWND found under cursor at {}", cursor_position);
        return;
      }
      if !Self::can_resize_window(target_hwnd) {
        debug!("Cannot resize window with HWND: {:?}", target_hwnd);
        return;
      }
      let mut window_rect = RECT::default();
      if GetWindowRect(target_hwnd, &mut window_rect).is_err() {
        error!("Failed to get window rect for HWND: {:?}", target_hwnd);
        return;
      }
      let window_rect = Rect::from(window_rect);
      if !SetForegroundWindow(target_hwnd).as_bool() {
        warn!("Failed to set foreground window to w#{:?}", target_hwnd.0);
      }
      let resize_mode = Self::determine_resize_mode(cursor_position, &window_rect);
      let window_handle = WindowHandle::from(target_hwnd);
      if let Ok(mut resize_state) = get_resize_state().lock() {
        resize_state.set(cursor_position, window_handle, window_rect, resize_mode);
        IS_RESIZING.store(true, Ordering::Relaxed);
        debug!("Started resizing in [{:?}] mode", resize_mode);
      }
    }
  }

  fn do_resize(cursor_point: POINT) {
    let resize_state = get_resize_state();
    let resize_guard = match resize_state.lock() {
      Ok(guard) => guard,
      Err(_) => return,
    };
    if !IS_RESIZING.load(Ordering::Relaxed) {
      debug!(
        "Not resizing, ignoring mouse move at ({}, {})",
        cursor_point.x, cursor_point.y
      );
      return;
    }
    let current_cursor = Point::from(cursor_point);
    let cursor_start_position = resize_guard.get_cursor_start_position();
    let delta_x = current_cursor.x() - cursor_start_position.x();
    let delta_y = current_cursor.y() - cursor_start_position.y();
    let window_hwnd = match resize_guard.get_window_handle() {
      Some(handle) => handle.as_hwnd(),
      None => {
        error!("Failed to retrieve the window handle for resizing, ignoring operation...");
        return;
      }
    };
    let resize_mode = resize_guard.get_resize_mode();
    let rect = resize_guard.get_window_start_rect();
    let (new_left, new_top, new_width, new_height) = match resize_mode {
      ResizeMode::BottomRight => {
        let new_width = (rect.right - rect.left) + delta_x;
        let new_height = (rect.bottom - rect.top) + delta_y;
        (rect.left, rect.top, new_width, new_height)
      }
      ResizeMode::TopLeft => {
        let new_left = rect.left + delta_x;
        let new_top = rect.top + delta_y;
        let new_width = (rect.right - rect.left) - delta_x;
        let new_height = (rect.bottom - rect.top) - delta_y;
        (new_left, new_top, new_width, new_height)
      }
      ResizeMode::TopRight => {
        let new_top = rect.top + delta_y;
        let new_width = (rect.right - rect.left) + delta_x;
        let new_height = (rect.bottom - rect.top) - delta_y;
        (rect.left, new_top, new_width, new_height)
      }
      ResizeMode::BottomLeft => {
        let new_left = rect.left + delta_x;
        let new_width = (rect.right - rect.left) - delta_x;
        let new_height = (rect.bottom - rect.top) + delta_y;
        (new_left, rect.top, new_width, new_height)
      }
    };
    drop(resize_guard);
    let min_width = 200;
    let min_height = 50;
    let final_width = new_width.max(min_width);
    let final_height = new_height.max(min_height);
    trace!(
      "Resizing window to ({}, {}) with size {}x{}",
      new_left, new_top, final_width, final_height
    );

    unsafe {
      if let Err(err) = SetWindowPos(
        window_hwnd,
        None,
        new_left,
        new_top,
        final_width,
        final_height,
        SWP_NOZORDER | SWP_NOACTIVATE,
      ) {
        error!("Failed to resize window: {}", err);
      }
    }
  }

  fn finish_resizing() {
    if let Ok(mut resize_state) = get_resize_state().lock() {
      resize_state.reset();
      IS_RESIZING.store(false, Ordering::Relaxed);
    }
  }

  fn determine_resize_mode(cursor_position: Point, window_rect: &Rect) -> ResizeMode {
    let distance_to_left = (cursor_position.x() - window_rect.left).abs();
    let distance_to_right = (cursor_position.x() - window_rect.right).abs();
    let distance_to_top = (cursor_position.y() - window_rect.top).abs();
    let distance_to_bottom = (cursor_position.y() - window_rect.bottom).abs();
    let is_closer_to_left = distance_to_left < distance_to_right;
    let is_closer_to_top = distance_to_top < distance_to_bottom;
    match (is_closer_to_left, is_closer_to_top) {
      (true, true) => ResizeMode::TopLeft,
      (false, true) => ResizeMode::TopRight,
      (true, false) => ResizeMode::BottomLeft,
      (false, false) => ResizeMode::BottomRight,
    }
  }

  fn can_resize_window(window: HWND) -> bool {
    unsafe {
      // Check if window has WS_THICKFRAME style (resizable)
      let style = GetWindowLongW(window, GWL_STYLE) as u32;
      let has_thick_frame = (style & WS_THICKFRAME.0) != 0;
      if !has_thick_frame {
        return false;
      }
    }

    Self::can_move_window(window)
  }

  /// Retrieves the top-level `HWND` for a given `HWND`.
  fn get_top_level_hwnd(mut window: HWND) -> HWND {
    unsafe {
      while !window.0.is_null() {
        let parent = GetParent(window);
        if parent.is_err() {
          break;
        }
        let parent = parent.expect("Failed to get parent of window");
        if parent.0.is_null() {
          break;
        }
        window = parent;
      }

      window
    }
  }
}

impl Drop for WindowsApiForDragging {
  fn drop(&mut self) {
    Self::uninstall_mouse_hook();
    if let Some(keyboard_hook) = self.keyboard_hook_handle {
      unsafe {
        if let Err(err) = UnhookWindowsHookEx(keyboard_hook) {
          error!("Failed to unhook keyboard hook: {}", err);
        }
      }
    }
  }
}

fn get_drag_state() -> &'static Arc<Mutex<DragState>> {
  DRAG_STATE.get_or_init(|| Arc::new(Mutex::new(DragState::default())))
}

fn get_resize_state() -> &'static Arc<Mutex<ResizeState>> {
  RESIZE_STATE.get_or_init(|| Arc::new(Mutex::new(ResizeState::default())))
}
