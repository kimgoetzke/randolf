use crate::utils::{Direction, Monitor, print_monitor_layout_to_canvas};

pub struct Monitors {
  monitors: Vec<Monitor>,
}

impl Monitors {
  pub fn from(mut monitors: Vec<Monitor>) -> Self {
    monitors.sort_by(|a, b| a.handle.cmp(&b.handle));
    Self { monitors }
  }

  pub fn get(&self, direction: Direction, reference_handle: isize) -> Option<&Monitor> {
    let monitor = self.get_by_handle(reference_handle)?;

    self.get_direction_of(monitor, direction)
  }

  pub fn get_by_handle(&self, handle: isize) -> Option<&Monitor> {
    self.monitors.iter().find(|m| m.handle == handle)
  }

  fn get_direction_of(&self, reference: &Monitor, direction: Direction) -> Option<&Monitor> {
    let mut left: Option<&Monitor> = None;
    let mut closest_distance = f64::MAX;
    for m in &self.monitors {
      if m.is_in_direction_of(reference, direction) {
        let distance = reference.center.distance_to(&m.center);
        if distance < closest_distance {
          closest_distance = distance;
          left = Some(m);
        }
      }
    }

    left
  }

  pub fn get_all(&self) -> Vec<&Monitor> {
    self.monitors.iter().collect()
  }

  pub fn print_layout(&self) {
    print_monitor_layout_to_canvas(&self.monitors);
  }
}

#[cfg(test)]
mod tests {
  use crate::utils::{Direction, Monitor, Monitors, Rect};

  #[test]
  fn from_sorts_monitors_by_handle() {
    let monitor1 = Monitor::new_test(2, Rect::new(0, 0, 1920, 1080));
    let monitor2 = Monitor::new_test(1, Rect::new(1920, 0, 3840, 1080));
    let monitors = Monitors::from(vec![monitor1.clone(), monitor2.clone()]);

    assert_eq!(monitors.get_all(), vec![&monitor2, &monitor1]);
  }

  #[test]
  fn get_returns_monitor_in_specified_direction() {
    let monitor1 = Monitor::new_test(1, Rect::new(0, 0, 1920, 1080));
    let monitor2 = Monitor::new_test(2, Rect::new(1920, 0, 3840, 1080));
    let monitors = Monitors::from(vec![monitor1.clone(), monitor2.clone()]);

    let result = monitors.get(Direction::Right, monitor1.handle);

    assert_eq!(result, Some(&monitor2));
  }

  #[test]
  fn get_returns_none_if_no_monitor_in_direction() {
    let monitor1 = Monitor::new_test(1, Rect::new(0, 0, 1920, 1080));
    let monitors = Monitors::from(vec![monitor1.clone()]);

    let result = monitors.get(Direction::Right, monitor1.handle);

    assert!(result.is_none());
  }

  #[test]
  fn get_by_handle_returns_correct_monitor() {
    let monitor1 = Monitor::new_test(1, Rect::new(0, 0, 1920, 1080));
    let monitor2 = Monitor::new_test(2, Rect::new(1920, 0, 3840, 1080));
    let monitors = Monitors::from(vec![monitor1.clone(), monitor2.clone()]);

    let result = monitors.get_by_handle(2);

    assert_eq!(result, Some(&monitor2));
  }

  #[test]
  fn get_by_handle_returns_none_for_invalid_handle() {
    let monitor1 = Monitor::new_test(1, Rect::new(0, 0, 1920, 1080));
    let monitors = Monitors::from(vec![monitor1.clone()]);

    let result = monitors.get_by_handle(99);

    assert!(result.is_none());
  }

  #[test]
  fn get_all_returns_all_monitors() {
    let monitor1 = Monitor::new_test(1, Rect::new(0, 0, 1920, 1080));
    let monitor2 = Monitor::new_test(2, Rect::new(1920, 0, 3840, 1080));
    let monitors = Monitors::from(vec![monitor1.clone(), monitor2.clone()]);

    let result = monitors.get_all();

    assert_eq!(result, vec![&monitor1, &monitor2]);
  }
}
