use crate::common::Command;

/// Describes whether a hotkey callback should reach the main command loop.
pub(super) enum HotkeyOutcome {
  /// Carries an accepted hotkey command to the main loop.
  Accepted(Command),
  /// Marks a repeated key-down event that must not be executed again.
  Suppressed,
}
