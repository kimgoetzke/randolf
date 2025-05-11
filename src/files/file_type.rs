/// The type of file to be managed by the `FileManager`. Determines the directory in which the file will be created.
/// See https://crates.io/crates/directories for more information.
pub enum FileType {
  /// The file will be created in the config directory i.e. `%APPDATA%` or `AppData\Roaming\`.
  Config,
  /// The file will be created in the data directory i.e. `%LOCALAPPDATA%` or `AppData\Local\`.
  Data,
}
