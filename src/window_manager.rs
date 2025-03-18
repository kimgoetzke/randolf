use crate::{EXTRA_Y_PADDING, native_api};
use std::collections::HashMap;
use winapi::shared::windef::{HWND, POINT, RECT};
use winapi::um::winuser::{MONITORINFO, SW_SHOWNORMAL, SendMessageW, SetWindowPlacement, WINDOWPLACEMENT, WM_PAINT};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT_FLAGS;

const TOLERANCE_IN_PX: i32 = 4;

pub(crate) struct WindowManager {
  known_windows: HashMap<String, WINDOWPLACEMENT>,
}

impl WindowManager {
  pub fn new() -> Self {
    Self {
      known_windows: HashMap::new(),
    }
  }

  pub fn near_maximise_active_window(&mut self) {
    info!("Hotkey has been pressed...");
    let Some(window) = native_api::get_foreground_window() else {
      return;
    };
    let Some(placement) = native_api::get_window_placement(window) else {
      return;
    };
    let Some(monitor_info) = native_api::get_monitor_info(window) else {
      return;
    };

    match is_near_maximized(&placement, window, monitor_info) {
      true => restore_previous_placement(&self.known_windows, window),
      false => {
        add_or_update_previous_placement(&mut self.known_windows, window, placement);
        near_maximize_window(window, 30);
      }
    }
  }
}

fn restore_previous_placement(known_windows: &HashMap<String, WINDOWPLACEMENT>, window: HWND) {
  let window_id = format!("{:?}", window);
  if let Some(previous_placement) = known_windows.get(&window_id) {
    info!("Restoring previous placement for #{}", window_id);
    native_api::restore_window_placement(window, previous_placement);
  } else {
    warn!("No previous placement found for #{}", window_id);
  }
}

fn add_or_update_previous_placement(
  known_windows: &mut HashMap<String, WINDOWPLACEMENT>,
  window: HWND,
  placement: WINDOWPLACEMENT,
) {
  let window_id = format!("{:?}", window);
  if known_windows.contains_key(&window_id) {
    known_windows.remove(&window_id);
    debug!(
      "Removing previous placement for window #{} so that a new value can be added",
      window_id
    );
  }

  known_windows.insert(window_id.clone(), placement);
  debug!("Adding/updating previous placement for window #{}", window_id);
}

fn near_maximize_window(window: HWND, margin: i32) {
  info!("Near-maximizing #{:?}", window);
  // Get the monitor working area for the window
  let monitor_info = match native_api::get_monitor_info(window) {
    Some(value) => value,
    None => return,
  };
  let work_area = monitor_info.rcWork;

  // Maximize first to get animation effect
  native_api::maximise_window(window);

  // Calculate new window size with padding
  let new_x = work_area.left + margin;
  let new_y = work_area.top + margin + EXTRA_Y_PADDING;
  let new_width = work_area.right - work_area.left - margin * 2;
  let new_height = work_area.bottom - work_area.top - margin * 2 - EXTRA_Y_PADDING;

  // Define the new window placement
  let placement = WINDOWPLACEMENT {
    length: size_of::<WINDOWPLACEMENT>() as u32,
    flags: WINDOWPLACEMENT_FLAGS(0).0,
    showCmd: SW_SHOWNORMAL as u32,
    ptMaxPosition: POINT { x: 0, y: 0 },
    ptMinPosition: POINT { x: -1, y: -1 },
    rcNormalPosition: RECT {
      left: new_x,
      top: new_y,
      right: new_x + new_width,
      bottom: new_y + new_height,
    },
  };

  // Update window placement
  unsafe {
    if SetWindowPlacement(window, &placement) == 0 {
      warn!("Failed to set window placement for #{:?}", window);
    }

    // Force a repaint
    SendMessageW(window, WM_PAINT, WPARAM(0).0, LPARAM(0).0);
  }
}

fn is_near_maximized(placement: &WINDOWPLACEMENT, window: HWND, monitor_info: MONITORINFO) -> bool {
  let work_area = monitor_info.rcWork;
  let expected_x = work_area.left + 30;
  let expected_y = work_area.top + 30 + EXTRA_Y_PADDING;
  let expected_width = work_area.right - work_area.left - 30 * 2;
  let expected_height = work_area.bottom - work_area.top - 30 * 2 - EXTRA_Y_PADDING;
  let rc = placement.rcNormalPosition;
  let result = (rc.left - expected_x).abs() <= TOLERANCE_IN_PX
    && (rc.top - expected_y).abs() <= TOLERANCE_IN_PX
    && (rc.right - rc.left - expected_width).abs() <= TOLERANCE_IN_PX
    && (rc.bottom - rc.top - expected_height).abs() <= TOLERANCE_IN_PX;

  debug!(
    "Expected size of #{:?}: ({},{})x({},{})",
    window, expected_x, expected_y, expected_width, expected_height
  );
  debug!(
    "Actual size of #{:?}: ({},{})x({},{})",
    window,
    rc.left,
    rc.top,
    rc.right - rc.left,
    rc.bottom - rc.top
  );
  debug!(
    "#{:?} {} near-maximized (tolerance: {})",
    window,
    if result { "is currently" } else { "is currently NOT" },
    TOLERANCE_IN_PX
  );

  result
}
