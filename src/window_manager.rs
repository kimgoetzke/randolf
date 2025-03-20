use crate::native_api;
use crate::native_api::update_window_placement_and_force_repaint;
use std::collections::HashMap;
use winapi::shared::windef::{HWND, POINT, RECT};
use winapi::um::winuser::{MONITORINFO, SW_SHOWNORMAL, WINDOWPLACEMENT};
use windows::Win32::UI::WindowsAndMessaging::WINDOWPLACEMENT_FLAGS;

const TOLERANCE_IN_PX: i32 = 4;
const DEFAULT_MARGIN: i32 = 25;

pub(crate) struct WindowManager {
  known_windows: HashMap<String, WINDOWPLACEMENT>,
}

struct Sizing {
  x: i32,
  y: i32,
  width: i32,
  height: i32,
}

// TODO: Add feature where pressing hotkey to move window to any side twice will move it to the next monitor in that
//  direction, if available
impl WindowManager {
  pub fn new() -> Self {
    Self {
      known_windows: HashMap::new(),
    }
  }

  pub fn near_maximise_or_restore(&mut self) {
    info!("Hotkey pressed - action: near-maximise window");
    let (window, placement, monitor_info) = match get_window_and_monitor_info() {
      Some(value) => value,
      None => return,
    };

    match is_near_maximized(&placement, window, monitor_info) {
      true => restore_previous_placement(&self.known_windows, window),
      false => {
        add_or_update_previous_placement(&mut self.known_windows, window, placement);
        near_maximize_window(window, monitor_info, DEFAULT_MARGIN);
      }
    }
  }

  pub fn move_to_right_half_of_screen(&mut self) {
    info!("Hotkey pressed - action: move window to right half of screen");
    let (window, _, monitor_info) = match get_window_and_monitor_info() {
      Some(value) => value,
      None => return,
    };

    // Resize the window to the expected size
    let work_area = monitor_info.rcWork;
    let sizing = Sizing {
      x: work_area.left + (work_area.right - work_area.left) / 2 + DEFAULT_MARGIN / 2,
      y: work_area.top + DEFAULT_MARGIN,
      width: (work_area.right - work_area.left) / 2 - DEFAULT_MARGIN - DEFAULT_MARGIN / 2,
      height: work_area.bottom - work_area.top - DEFAULT_MARGIN * 2,
    };
    execute_window_resizing(window, sizing);
  }

  pub fn move_to_top_half_of_screen(&mut self) {
    info!("Hotkey pressed - action: move window to top half of screen");
    let (window, _, monitor_info) = match get_window_and_monitor_info() {
      Some(value) => value,
      None => return,
    };

    // Resize the window to the expected size
    let work_area = monitor_info.rcWork;
    let sizing = Sizing {
      x: work_area.left + DEFAULT_MARGIN,
      y: work_area.top + DEFAULT_MARGIN,
      width: work_area.right - work_area.left - DEFAULT_MARGIN * 2,
      height: (work_area.bottom - work_area.top) / 2 - DEFAULT_MARGIN - DEFAULT_MARGIN / 2,
    };
    execute_window_resizing(window, sizing);
  }

  pub fn move_to_bottom_half_of_screen(&mut self) {
    info!("Hotkey pressed - action: move window to bottom half of screen");
    let (window, _, monitor_info) = match get_window_and_monitor_info() {
      Some(value) => value,
      None => return,
    };

    // Resize the window to the expected size
    let work_area = monitor_info.rcWork;
    let sizing = Sizing {
      x: work_area.left + DEFAULT_MARGIN,
      y: work_area.top + (work_area.bottom - work_area.top) / 2 + DEFAULT_MARGIN / 2,
      width: work_area.right - work_area.left - DEFAULT_MARGIN * 2,
      height: (work_area.bottom - work_area.top) / 2 - DEFAULT_MARGIN - DEFAULT_MARGIN / 2,
    };
    execute_window_resizing(window, sizing);
  }

  pub fn move_to_left_half_of_screen(&mut self) {
    info!("Hotkey pressed - action: move window to left half of screen");
    let (window, _, monitor_info) = match get_window_and_monitor_info() {
      Some(value) => value,
      None => return,
    };

    // Resize the window to the expected size
    let work_area = monitor_info.rcWork;
    let sizing = Sizing {
      x: work_area.left + DEFAULT_MARGIN,
      y: work_area.top + DEFAULT_MARGIN,
      width: (work_area.right - work_area.left) / 2 - DEFAULT_MARGIN - DEFAULT_MARGIN / 2,
      height: work_area.bottom - work_area.top - DEFAULT_MARGIN * 2,
    };
    execute_window_resizing(window, sizing);
  }
}

fn get_window_and_monitor_info() -> Option<(HWND, WINDOWPLACEMENT, MONITORINFO)> {
  let window = native_api::get_foreground_window()?;
  let placement = native_api::get_window_placement(window)?;
  let monitor_info = native_api::get_monitor_info(window)?;
  Some((window, placement, monitor_info))
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

fn near_maximize_window(window: HWND, monitor_info: MONITORINFO, margin: i32) {
  info!("Near-maximizing #{:?}", window);

  // Maximize first to get the animation effect
  native_api::maximise_window(window);

  // Resize the window to the expected size
  let work_area = monitor_info.rcWork;
  let sizing = Sizing {
    x: work_area.left + margin,
    y: work_area.top + margin,
    width: work_area.right - work_area.left - margin * 2,
    height: work_area.bottom - work_area.top - margin * 2,
  };
  execute_window_resizing(window, sizing);
}

fn is_near_maximized(placement: &WINDOWPLACEMENT, window: HWND, monitor_info: MONITORINFO) -> bool {
  let work_area = monitor_info.rcWork;
  let expected_x = work_area.left + 30;
  let expected_y = work_area.top + 30;
  let expected_width = work_area.right - work_area.left - 30 * 2;
  let expected_height = work_area.bottom - work_area.top - 30 * 2;
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

fn execute_window_resizing(window: HWND, sizing: Sizing) {
  let placement = WINDOWPLACEMENT {
    length: size_of::<WINDOWPLACEMENT>() as u32,
    flags: WINDOWPLACEMENT_FLAGS(0).0,
    showCmd: SW_SHOWNORMAL as u32,
    ptMaxPosition: POINT { x: 0, y: 0 },
    ptMinPosition: POINT { x: -1, y: -1 },
    rcNormalPosition: RECT {
      left: sizing.x,
      top: sizing.y,
      right: sizing.x + sizing.width,
      bottom: sizing.y + sizing.height,
    },
  };
  update_window_placement_and_force_repaint(window, &placement);
}
