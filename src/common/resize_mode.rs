#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ResizeMode {
  TopRight,
  #[default]
  BottomRight,
  BottomLeft,
  TopLeft,
}
