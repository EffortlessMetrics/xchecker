/// Enhance error reporting for common failure scenarios
pub(super) fn enhance_error_context(error: &anyhow::Error) -> Option<Vec<String>> {
    let error_str = error.to_string();

    if error_str.contains("Failed to create orchestrator") {
        Some(vec![
            "Check that the current directory is writable".to_string(),
            "Ensure sufficient disk space is available".to_string(),
            "Verify directory permissions".to_string(),
            "Try running from a different directory".to_string(),
        ])
    } else if error_str.contains("Failed to execute") {
        Some(vec![
            "Check the spec ID is valid and doesn't contain special characters".to_string(),
            "Verify Claude CLI is installed and accessible".to_string(),
            "Try running with --dry-run to test configuration".to_string(),
            "Check your internet connection if using Claude API".to_string(),
        ])
    } else if error_str.contains("Permission denied") {
        Some(vec![
            "Check file and directory permissions".to_string(),
            "Ensure you have write access to the current directory".to_string(),
            "Try running from your home directory or a writable location".to_string(),
        ])
    } else if error_str.contains("No such file or directory") {
        Some(vec![
            "Verify the specified paths exist".to_string(),
            "Check that you're running from the correct directory".to_string(),
            "Ensure all required files are present".to_string(),
        ])
    } else {
        None
    }
}
