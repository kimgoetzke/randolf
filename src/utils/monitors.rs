use crate::utils::{Direction, Monitor, print_monitor_layout_to_canvas};

pub struct Monitors {
  monitors: Vec<Monitor>,
}

impl Monitors {
  pub fn from(monitors: Vec<Monitor>) -> Self {
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

  #[allow(dead_code)]
  pub fn print_layout(&self) {
    print_monitor_layout_to_canvas(&self.monitors);
  }
}
