use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Accepts one key-down event, then blocks repeat events until the matching key release.
///
/// Clones share atomic state so the hotkey callback and release hook can run on separate threads without locking.
#[derive(Clone, Default)]
pub(super) struct PressLatch {
  pressed: Arc<AtomicBool>,
}

impl PressLatch {
  /// Marks the key as pressed, returning `true` only if it was previously released.
  pub(super) fn try_press(&self) -> bool {
    !self.pressed.swap(true, Ordering::Acquire)
  }

  /// Marks the key as released so the next key-down event is accepted.
  pub(super) fn release(&self) {
    self.pressed.store(false, Ordering::Release);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn accepts_once_until_rearmed() {
    let latch = PressLatch::default();

    assert!(latch.try_press());
    assert!(!latch.try_press());

    latch.release();

    assert!(latch.try_press());
  }
}
