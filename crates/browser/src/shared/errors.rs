use rocky_core::JobError;

pub fn to_job_error(e: impl std::fmt::Display, action: &str) -> JobError {
    let s = e.to_string();
    if s.contains("timeout") || s.contains("Timeout") {
        JobError::timeout_error(format!("{} timed out: {}", action, s))
    } else if s.contains("navigation") || s.contains("Navigation") {
        JobError::navigation_error(format!("{} navigation failed: {}", action, s))
    } else if s.contains("not found") || s.contains("null") {
        JobError::element_not_found(format!("{}: {}", action, s))
    } else {
        JobError::browser_error(format!("{} failed: {}", action, s))
    }
}