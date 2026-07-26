/// A monitor-relative scrolling layout width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WidthPreset {
  Quarter,
  Third,
  Half,
  TwoThirds,
  ThreeQuarters,
  NearMaximised,
}

const WIDTH_PRESETS: [WidthPreset; 6] = [
  WidthPreset::Quarter,
  WidthPreset::Third,
  WidthPreset::Half,
  WidthPreset::TwoThirds,
  WidthPreset::ThreeQuarters,
  WidthPreset::NearMaximised,
];

impl WidthPreset {
  /// Selects the preset closest to an observed pixel width.
  pub(crate) fn nearest(observed_width: i32, usable_width: i32) -> Self {
    WIDTH_PRESETS
      .into_iter()
      .min_by_key(|preset| preset.width(usable_width).abs_diff(observed_width))
      .unwrap_or(Self::NearMaximised)
  }

  /// Returns the next narrower preset, clamped at one quarter.
  pub(crate) fn narrower(self) -> Self {
    let index = WIDTH_PRESETS.iter().position(|preset| *preset == self).unwrap_or(0);
    WIDTH_PRESETS[index.saturating_sub(1)]
  }

  /// Returns the next wider preset, clamped at near-maximised.
  pub(crate) fn wider(self) -> Self {
    let index = WIDTH_PRESETS.iter().position(|preset| *preset == self).unwrap_or(0);
    WIDTH_PRESETS[(index + 1).min(WIDTH_PRESETS.len() - 1)]
  }

  /// Calculates this preset's pixel width.
  pub(crate) fn width(self, usable_width: i32) -> i32 {
    match self {
      Self::Quarter => usable_width / 4,
      Self::Third => usable_width / 3,
      Self::Half => usable_width / 2,
      Self::TwoThirds => fraction_width(usable_width, 2, 3),
      Self::ThreeQuarters => fraction_width(usable_width, 3, 4),
      Self::NearMaximised => usable_width,
    }
  }
}

fn fraction_width(usable_width: i32, numerator: i64, denominator: i64) -> i32 {
  let width = i64::from(usable_width) * numerator / denominator;
  i32::try_from(width).unwrap_or(if width.is_negative() { i32::MIN } else { i32::MAX })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn nearest_selects_each_monitor_relative_width_preset() {
    let usable_width = 1_200;
    let cases = [
      (300, WidthPreset::Quarter),
      (400, WidthPreset::Third),
      (600, WidthPreset::Half),
      (800, WidthPreset::TwoThirds),
      (900, WidthPreset::ThreeQuarters),
      (1_200, WidthPreset::NearMaximised),
    ];

    for (observed_width, expected) in cases {
      assert_eq!(WidthPreset::nearest(observed_width, usable_width), expected);
    }
  }

  #[test]
  fn narrower_traverses_every_width_preset_without_wrapping() {
    let cases = [
      (WidthPreset::Quarter, WidthPreset::Quarter),
      (WidthPreset::Third, WidthPreset::Quarter),
      (WidthPreset::Half, WidthPreset::Third),
      (WidthPreset::TwoThirds, WidthPreset::Half),
      (WidthPreset::ThreeQuarters, WidthPreset::TwoThirds),
      (WidthPreset::NearMaximised, WidthPreset::ThreeQuarters),
    ];

    for (preset, expected) in cases {
      assert_eq!(preset.narrower(), expected);
    }
  }

  #[test]
  fn wider_traverses_every_width_preset_without_wrapping() {
    let cases = [
      (WidthPreset::Quarter, WidthPreset::Third),
      (WidthPreset::Third, WidthPreset::Half),
      (WidthPreset::Half, WidthPreset::TwoThirds),
      (WidthPreset::TwoThirds, WidthPreset::ThreeQuarters),
      (WidthPreset::ThreeQuarters, WidthPreset::NearMaximised),
      (WidthPreset::NearMaximised, WidthPreset::NearMaximised),
    ];

    for (preset, expected) in cases {
      assert_eq!(preset.wider(), expected);
    }
  }

  #[test]
  fn width_calculates_every_preset() {
    let usable_width = 1_200;
    let cases = [
      (WidthPreset::Quarter, 300),
      (WidthPreset::Third, 400),
      (WidthPreset::Half, 600),
      (WidthPreset::TwoThirds, 800),
      (WidthPreset::ThreeQuarters, 900),
      (WidthPreset::NearMaximised, 1_200),
    ];

    for (preset, expected) in cases {
      assert_eq!(preset.width(usable_width), expected);
    }
  }

  #[test]
  fn width_calculates_fractional_widths_without_intermediate_overflow() {
    assert_eq!(WidthPreset::TwoThirds.width(i32::MAX), 1_431_655_764);
    assert_eq!(WidthPreset::ThreeQuarters.width(i32::MAX), 1_610_612_735);
  }
}
