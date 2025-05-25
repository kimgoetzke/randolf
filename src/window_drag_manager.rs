use crate::api::WindowsApi;
use crate::common::{Command, Point};
use crossbeam_channel::Sender;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
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

pub struct WindowDragManager<T: WindowsApi> {
  windows_api: T,
  keyboard_hook_handle: Option<HHOOK>,
  command_sender: Sender<Command>,
}

impl<T: WindowsApi> WindowDragManager<T> {
  pub fn new(windows_api: T, sender: Sender<Command>) -> Self {
    Self {
      windows_api,
      keyboard_hook_handle: None,
      command_sender: sender,
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

  // TODO: Find a way to inject windows_api and AtomicBools into the callbacks, then refactor everything
  extern "system" fn keyboard_callback(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    unsafe {
      if n_code == HC_ACTION as i32 {
        let keyboard_data = *(l_param.0 as *const KBDLLHOOKSTRUCT);
        let vk_code = keyboard_data.vkCode;
        let is_window_key = vk_code == VK_LWIN.0 as u32 || vk_code == VK_RWIN.0 as u32;
        if is_window_key {
          let is_pressed = (w_param.0 as u32) == WM_KEYDOWN || (w_param.0 as u32) == WM_SYSKEYDOWN;
          if is_pressed != IS_WIN_KEY_PRESSED.load(Ordering::Relaxed) {
            debug!("Win key [{}] {}", vk_code, if is_pressed { "pressed" } else { "released" });
            IS_WIN_KEY_PRESSED.store(is_pressed, Ordering::Relaxed);
            if is_pressed {
              Self::install_mouse_hook();
            } else {
              if IS_DRAGGING.load(Ordering::Relaxed) {
                Self::handle_drag_end();
              }
              if IS_RESIZING.load(Ordering::Relaxed) {
                Self::handle_resize_end();
              }
              Self::uninstall_mouse_hook();
            }
          }
        }
      }

      CallNextHookEx(None, n_code, w_param, l_param)
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
        debug!("Mouse hook installed");
      } else {
        error!("Failed to install mouse hook");
      }
    }
  }

  fn uninstall_mouse_hook() {
    unsafe {
      let hook_pointer = MOUSE_HOOK_HANDLE.swap(std::ptr::null_mut(), Ordering::Relaxed);
      if !hook_pointer.is_null() {
        let hook = HHOOK(hook_pointer);
        if let Err(err) = UnhookWindowsHookEx(hook) {
          error!("Failed to unhook mouse hook: {}", err);
        } else {
          debug!("Mouse hook uninstalled");
        }
      }
    }
  }

  // TODO: Find a way to inject windows_api and AtomicBools into the callbacks, then refactor everything
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
          Self::handle_drag_start(cursor_position);
          return LRESULT(1);
        }
        WM_LBUTTONUP => {
          if Self::is_dragging_active() {
            debug!("Win key + left mouse button released, ending drag...",);
            Self::handle_drag_end();
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
          Self::handle_resize_start(cursor_position);
          return LRESULT(1);
        }
        WM_RBUTTONUP => {
          if Self::is_resizing_active() {
            debug!("Win key + right mouse button released, ending window resizing...");
            Self::handle_resize_end();
            return LRESULT(1);
          }
        }
        WM_MOUSEMOVE => {
          if Self::is_dragging_active() {
            let mouse_low_level_hook_struct = *(l_param.0 as *const MSLLHOOKSTRUCT);
            Self::handle_drag_move(mouse_low_level_hook_struct.pt);
            return CallNextHookEx(None, n_code, w_param, l_param);
          } else if Self::is_resizing_active() {
            let mouse_low_level_hook_struct = *(l_param.0 as *const MSLLHOOKSTRUCT);
            Self::handle_resize_move(mouse_low_level_hook_struct.pt);
            return CallNextHookEx(None, n_code, w_param, l_param);
          }
        }
        _ => return CallNextHookEx(None, n_code, w_param, l_param),
      }

      CallNextHookEx(None, n_code, w_param, l_param)
    }
  }

  fn handle_drag_start(cursor_position: Point) {
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
      if let Ok(mut drag_state) = get_drag_state().lock() {
        let window_position = Point::new(window_rect.left, window_rect.top);
        drag_state.start_drag(cursor_position, target_hwnd, window_position);
        IS_DRAGGING.store(true, Ordering::Relaxed);
      }
    }
  }

  fn handle_drag_move(cursor_point: POINT) {
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
    let delta_x = cursor_point.x - drag_guard.drag_start_position.x();
    let delta_y = cursor_point.y - drag_guard.drag_start_position.y();
    let new_x = drag_guard.window_start_position.x() + delta_x;
    let new_y = drag_guard.window_start_position.y() + delta_y;
    let target_window = drag_guard.get_target_window();
    drop(drag_guard);

    trace!("Dragging window to ({}, {})", new_x, new_y);
    unsafe {
      if let Err(err) = SetWindowPos(
        target_window,
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

  fn handle_drag_end() {
    if let Ok(mut drag_state) = get_drag_state().lock() {
      drag_state.end_drag();
      IS_DRAGGING.store(false, Ordering::Relaxed);
    }
  }

  fn is_dragging_active() -> bool {
    IS_DRAGGING.load(Ordering::Relaxed)
  }

  fn is_resizing_active() -> bool {
    IS_RESIZING.load(Ordering::Relaxed)
  }

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

  fn handle_resize_start(cursor_position: Point) {
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
      let resize_mode = Self::determine_resize_mode(cursor_position, &window_rect);
      if let Ok(mut resize_state) = get_resize_state().lock() {
        resize_state.start_resize(cursor_position, target_hwnd, window_rect, resize_mode);
        IS_RESIZING.store(true, Ordering::Relaxed);
        debug!("Started resizing with mode: {:?}", resize_mode);
      }
    }
  }

  fn handle_resize_move(cursor_point: POINT) {
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
    let delta_x = current_cursor.x() - resize_guard.resize_start_position.x();
    let delta_y = current_cursor.y() - resize_guard.resize_start_position.y();
    let (top_left, bottom_right) = resize_guard.window_start_rect;
    let target_window = resize_guard.get_target_window();
    let resize_mode = resize_guard.resize_mode;

    let (new_left, new_top, new_width, new_height) = match resize_mode {
      ResizeMode::BottomRight => {
        let new_width = (bottom_right.x() - top_left.x()) + delta_x;
        let new_height = (bottom_right.y() - top_left.y()) + delta_y;
        (top_left.x(), top_left.y(), new_width, new_height)
      }
      ResizeMode::TopLeft => {
        let new_left = top_left.x() + delta_x;
        let new_top = top_left.y() + delta_y;
        let new_width = bottom_right.x() - new_left;
        let new_height = bottom_right.y() - new_top;
        (new_left, new_top, new_width, new_height)
      }
      ResizeMode::TopRight => {
        let new_top = top_left.y() + delta_y;
        let new_width = (bottom_right.x() - top_left.x()) + delta_x;
        let new_height = bottom_right.y() - new_top;
        (top_left.x(), new_top, new_width, new_height)
      }
      ResizeMode::BottomLeft => {
        let new_left = top_left.x() + delta_x;
        let new_width = bottom_right.x() - new_left;
        let new_height = (bottom_right.y() - top_left.y()) + delta_y;
        (new_left, top_left.y(), new_width, new_height)
      }
    };
    drop(resize_guard);
    let min_width = 100;
    let min_height = 50;
    let final_width = new_width.max(min_width);
    let final_height = new_height.max(min_height);
    trace!(
      "Resizing window to ({}, {}) with size {}x{}",
      new_left, new_top, final_width, final_height
    );

    unsafe {
      if let Err(err) = SetWindowPos(
        target_window,
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

  fn handle_resize_end() {
    if let Ok(mut resize_state) = get_resize_state().lock() {
      resize_state.end_resize();
      IS_RESIZING.store(false, Ordering::Relaxed);
    }
  }

  fn determine_resize_mode(cursor_position: Point, window_rect: &RECT) -> ResizeMode {
    let dist_left = (cursor_position.x() - window_rect.left).abs();
    let dist_right = (cursor_position.x() - window_rect.right).abs();
    let dist_top = (cursor_position.y() - window_rect.top).abs();
    let dist_bottom = (cursor_position.y() - window_rect.bottom).abs();
    let is_closer_to_left = dist_left < dist_right;
    let is_closer_to_top = dist_top < dist_bottom;
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
}

impl<T: WindowsApi> Drop for WindowDragManager<T> {
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
  DRAG_STATE.get_or_init(|| {
    Arc::new(Mutex::new(DragState {
      drag_start_position: Point::default(),
      window_start_position: Point::default(),
      target_window_id: 0,
    }))
  })
}

fn get_resize_state() -> &'static Arc<Mutex<ResizeState>> {
  RESIZE_STATE.get_or_init(|| {
    Arc::new(Mutex::new(ResizeState {
      resize_start_position: Point::default(),
      window_start_rect: (Point::default(), Point::default()),
      target_window_id: 0,
      resize_mode: ResizeMode::BottomRight,
    }))
  })
}

struct DragState {
  drag_start_position: Point,
  window_start_position: Point,
  target_window_id: isize,
}

impl DragState {
  fn start_drag(&mut self, cursor_position: Point, hwnd: HWND, window_position: Point) {
    self.drag_start_position = cursor_position;
    self.window_start_position = window_position;
    self.target_window_id = hwnd.0 as isize;
  }

  fn end_drag(&mut self) {
    self.drag_start_position = Point::default();
    self.window_start_position = Point::default();
    self.target_window_id = 0;
  }

  fn get_target_window(&self) -> HWND {
    HWND(self.target_window_id as *mut std::ffi::c_void)
  }
}

struct ResizeState {
  resize_start_position: Point,
  window_start_rect: (Point, Point), // (top_left, bottom_right)
  target_window_id: isize,
  resize_mode: ResizeMode,
}

#[derive(Clone, Copy, Debug)]
enum ResizeMode {
  BottomRight,
  TopLeft,
  TopRight,
  BottomLeft,
}

impl ResizeState {
  fn start_resize(&mut self, cursor_position: Point, hwnd: HWND, window_rect: RECT, resize_mode: ResizeMode) {
    self.resize_start_position = cursor_position;
    self.window_start_rect = (
      Point::new(window_rect.left, window_rect.top),
      Point::new(window_rect.right, window_rect.bottom),
    );
    self.target_window_id = hwnd.0 as isize;
    self.resize_mode = resize_mode;
  }

  fn end_resize(&mut self) {
    self.resize_start_position = Point::default();
    self.window_start_rect = (Point::default(), Point::default());
    self.target_window_id = 0;
  }

  fn get_target_window(&self) -> HWND {
    HWND(self.target_window_id as *mut std::ffi::c_void)
  }
}
