use crate::common::MonitorHandle;

pub fn id_to_string(id: &[u16; 32], handle: &MonitorHandle) -> String {
  let device_name = String::from_utf16_lossy(id).trim_end_matches('\0').to_string();
  if !device_name.is_empty() {
    device_name
  } else {
    format!("Unidentified Monitor {}", handle)
  }
}

pub fn id_to_string_or_panic(id: &[u16; 32]) -> String {
  let device_name = String::from_utf16_lossy(id).trim_end_matches('\0').to_string();
  if !device_name.is_empty() {
    device_name
  } else {
    panic!("Failed to convert ID to string");
  }
}
