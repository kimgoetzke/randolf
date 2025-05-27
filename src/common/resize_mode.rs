/// An enum that represents the way in which a window can be resized by the user. For example, `TopRight` means that
/// a window's top and right edges will be resized, while the bottom and left edges will remain fixed.
///
/// Only used for mouse-based resizing operations, *not* for any keyboard operations.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ResizeMode {
  TopRight,
  #[default]
  BottomRight,
  BottomLeft,
  TopLeft,
}
