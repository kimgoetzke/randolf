use crate::api::WindowsApi;
use crate::common::{Direction, MonitorInfo, Rect, Sizing, WindowHandle, WindowPlacement};
use crate::utils::MINIMUM_WINDOW_MARGIN;
use std::collections::HashMap;
use windows::Win32::UI::WindowsAndMessaging::SW_MAXIMIZE;

const REGULAR_TOLERANCE_IN_PX: i32 = 2;
pub(super) const DWM_TOLERANCE_IN_PX: i32 = 8;

/// Remembers window positions and applies Windows-aware sizing corrections.
#[derive(Default)]
pub(crate) struct Placement {
  pub(super) known_windows: HashMap<String, WindowPlacement>,
}

impl Placement {
  /// Near-maximises a window or restores the position saved before maximising it.
  pub(crate) fn near_maximise_or_restore<T: WindowsApi>(
    &mut self,
    api: &T,
    handle: WindowHandle,
    placement: WindowPlacement,
    monitor_info: MonitorInfo,
    margin: i32,
  ) {
    if self.is_near_maximised(api, &placement, &handle, &monitor_info, margin) {
      self.restore_previous(api, handle);
    } else {
      self.remember(handle, placement);
      self.near_maximise(api, handle, monitor_info, margin);
    }
  }

  /// Restores a window's last remembered position when one is available.
  pub(crate) fn restore_previous<T: WindowsApi>(&self, api: &T, handle: WindowHandle) {
    let window_id = format!("{:?}", handle.hwnd);
    if let Some(previous_placement) = self.known_windows.get(&window_id) {
      info!("Restoring previous placement for {}", window_id);
      api.do_restore_window_placement(handle, previous_placement.clone());
    } else {
      warn!("No previous placement found for {}", window_id);
    }
  }

  /// Reports whether a window fills its work area apart from the configured margin.
  pub(crate) fn is_near_maximised<T: WindowsApi>(
    &self,
    api: &T,
    placement: &WindowPlacement,
    handle: &WindowHandle,
    monitor_info: &MonitorInfo,
    margin: i32,
  ) -> bool {
    if placement.show_cmd == SW_MAXIMIZE.0 as u32 && margin < MINIMUM_WINDOW_MARGIN {
      debug!("{} is reported as maximised and margins are disabled", handle);
      return true;
    }

    let expected = Sizing::near_maximised(monitor_info.work_area, margin);
    if let Some(rect) = api.get_window_rect(*handle) {
      let result = is_sizing_within_tolerance(rect, &expected, REGULAR_TOLERANCE_IN_PX);
      log_actual_vs_expected(handle, &expected, rect);
      debug!(
        "{} {} near-maximised (tolerance: {})",
        handle,
        if result { "is currently" } else { "is currently NOT" },
        REGULAR_TOLERANCE_IN_PX
      );
      result
    } else {
      warn!("{} has no window rect, assuming currently NOT near-maximised", handle);
      false
    }
  }

  /// Reports whether a window fills three quarters of its work area in a direction.
  pub(crate) fn is_three_quarter_near_maximised<T: WindowsApi>(
    &self,
    api: &T,
    handle: &WindowHandle,
    monitor_info: &MonitorInfo,
    direction: Direction,
    margin: i32,
  ) -> bool {
    let expected = Sizing::three_quarter_near_maximised(monitor_info.work_area, direction, margin);
    if let Some(rect) = api.get_window_rect(*handle) {
      let result = is_sizing_within_tolerance(rect, &expected, REGULAR_TOLERANCE_IN_PX);
      debug!(
        "{} {} three-quarter near-maximised in [{:?}] direction (tolerance: {})",
        handle,
        if result { "is currently" } else { "is currently NOT" },
        direction,
        REGULAR_TOLERANCE_IN_PX
      );
      result
    } else {
      warn!(
        "{} has no window rect, assuming currently NOT three-quarter near-maximised",
        handle
      );
      false
    }
  }

  /// Expands a window to its work area while keeping the configured margin.
  pub(crate) fn near_maximise<T: WindowsApi>(&self, api: &T, handle: WindowHandle, monitor_info: MonitorInfo, margin: i32) {
    info!("Near-maximising {}", handle);

    // First maximise to get the animation effect
    api.do_maximise_window(handle);

    // Then resize the window to the expected size
    if margin >= MINIMUM_WINDOW_MARGIN {
      self.resize(api, handle, Sizing::near_maximised(monitor_info.work_area, margin), margin);
    }
  }

  /// Applies a size and corrects hidden Windows borders when margins are disabled.
  pub(crate) fn resize<T: WindowsApi>(&self, api: &T, handle: WindowHandle, sizing: Sizing, margin: i32) {
    api.set_window_placement_and_force_repaint(handle, WindowPlacement::new_from_sizing(sizing.clone()));
    self.correct_hidden_borders(api, handle, &sizing, margin);
  }

  fn correct_hidden_borders<T: WindowsApi>(&self, api: &T, handle: WindowHandle, sizing: &Sizing, margin: i32) {
    if margin == 0
      && let Some(rect) = api.get_extended_frame_bounds(handle).or_else(|| api.get_window_rect(handle))
      && let Some(compensating_rect) = calculate_compensating_rect_if_required(&rect, sizing)
    {
      api.set_window_position(handle, compensating_rect);
    }
  }

  /// Determines whether the given window placement matches the expected sizing. If margins are disabled, allows a
  /// small tolerance when comparing against the DWM extended frame bounds to account for shadows/rounded corners added
  /// by the OS.
  ///
  /// This extra check may be useful in all cases, but the Windows API behaviour is not sufficiently understood to apply
  /// it more broadly yet.
  pub(crate) fn is_of_expected_size<T: WindowsApi>(
    &self,
    api: &T,
    handle: WindowHandle,
    placement: &WindowPlacement,
    sizing: &Sizing,
    margin: i32,
  ) -> bool {
    let rect = placement.normal_position;
    let exact = rect.left == sizing.x
      && rect.top == sizing.y
      && rect.right - rect.left == sizing.width
      && rect.bottom - rect.top == sizing.height;
    if exact {
      log_actual_vs_expected(&handle, sizing, rect);
      debug!("{} is currently of expected size (exact placement match)", handle);
      return true;
    }

    if margin == 0
      && let Some(compensating_rect) = api.get_extended_frame_bounds(handle).or_else(|| api.get_window_rect(handle))
    {
      let matches = is_sizing_within_tolerance(compensating_rect, sizing, DWM_TOLERANCE_IN_PX);
      log_actual_vs_expected(&handle, sizing, compensating_rect);
      debug!(
        "{} {} of expected size (dwm_tolerance: {})",
        handle,
        if matches { "is currently" } else { "is currently NOT" },
        DWM_TOLERANCE_IN_PX
      );
      return matches;
    }

    log_actual_vs_expected(&handle, sizing, rect);
    debug!("{} is currently NOT of expected size (strict placement comparison)", handle);
    false
  }

  fn remember(&mut self, handle: WindowHandle, placement: WindowPlacement) {
    let window_id = format!("{:?}", handle.hwnd);
    if self.known_windows.remove(&window_id).is_some() {
      trace!(
        "Removing previous placement for window {} so that a new value can be added",
        handle
      );
    }
    self.known_windows.insert(window_id, placement);
    trace!("Adding/updating previous placement for window {}", handle);
  }
}

fn is_sizing_within_tolerance(rect: Rect, expected: &Sizing, tolerance: i32) -> bool {
  (rect.left - expected.x).abs() <= tolerance
    && (rect.top - expected.y).abs() <= tolerance
    && (rect.right - rect.left - expected.width).abs() <= tolerance
    && (rect.bottom - rect.top - expected.height).abs() <= tolerance
}

fn log_actual_vs_expected(handle: &WindowHandle, sizing: &Sizing, rect: Rect) {
  debug!(
    "Expected size of {}: ({},{})x({},{})",
    handle, sizing.x, sizing.y, sizing.width, sizing.height
  );
  debug!(
    "Actual size of {}: ({},{})x({},{})",
    handle,
    rect.left,
    rect.top,
    rect.right - rect.left,
    rect.bottom - rect.top
  );
}

fn calculate_compensating_rect_if_required(rect: &Rect, sizing: &Sizing) -> Option<Rect> {
  let requested_right = sizing.x + sizing.width;
  let left_inset = rect.left - sizing.x;
  let right_inset = requested_right - rect.right;
  if left_inset > 0 || right_inset > 0 {
    return Some(Rect::new(
      sizing.x - left_inset.max(0),
      sizing.y,
      requested_right + right_inset.max(0),
      sizing.y + sizing.height,
    ));
  }
  None
}
