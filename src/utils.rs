const CHAR_LIMIT: usize = 20;

pub fn truncated_str(s: &str) -> String {
  let char_count = s.chars().count();
  if char_count <= CHAR_LIMIT + CHAR_LIMIT {
    s.to_string()
  } else {
    let prefix: String = s.chars().take(CHAR_LIMIT).collect();
    let suffix: String = s.chars().skip(char_count - CHAR_LIMIT).collect();
    format!("{}...{}", prefix, suffix)
  }
}
