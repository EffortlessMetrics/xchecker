/// Result of NDJSON parsing from stdout
#[derive(Debug, Clone)]
pub enum NdjsonResult {
    /// Successfully found at least one valid JSON object (returns the last one)
    ValidJson(String),
    /// No valid JSON found, includes a tail excerpt for error reporting
    NoValidJson { tail_excerpt: String },
}

pub(crate) fn parse_ndjson(stdout: &str) -> NdjsonResult {
    let mut last_valid_json: Option<String> = None;

    // Parse line by line
    for line in stdout.lines() {
        let trimmed = line.trim();

        // Skip empty lines
        if trimmed.is_empty() {
            continue;
        }

        // Try to parse as JSON
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            // Valid JSON - store it as the last valid object
            // We serialize it back to ensure it's a valid JSON string
            if let Ok(json_str) = serde_json::to_string(&value) {
                last_valid_json = Some(json_str);
            }
        }
        // If parsing fails, ignore the line (it's noise)
    }

    // Return the last valid JSON if we found any
    if let Some(json) = last_valid_json {
        NdjsonResult::ValidJson(json)
    } else {
        // No valid JSON found - create a tail excerpt
        // Take up to 256 characters from the end of stdout
        let tail_excerpt = if stdout.len() <= 256 {
            stdout.to_string()
        } else {
            // Take the last 256 characters
            let start = stdout.len() - 256;
            stdout[start..].to_string()
        };

        NdjsonResult::NoValidJson { tail_excerpt }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // NDJSON parsing tests

    #[test]
    fn test_parse_ndjson_single_valid_json() {
        let stdout = r#"{"status": "success", "result": "done"}"#;
        let result = parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(json) => {
                let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
                assert_eq!(parsed["status"], "success");
                assert_eq!(parsed["result"], "done");
            }
            NdjsonResult::NoValidJson { .. } => panic!("Expected ValidJson"),
        }
    }

    #[test]
    fn test_parse_ndjson_multiple_valid_json_returns_last() {
        let stdout = r#"{"frame": 1}
{"frame": 2}
{"frame": 3}"#;
        let result = parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(json) => {
                let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
                assert_eq!(parsed["frame"], 3);
            }
            NdjsonResult::NoValidJson { .. } => panic!("Expected ValidJson"),
        }
    }

    #[test]
    fn test_parse_ndjson_interleaved_noise_and_json() {
        // AT-RUN-004: Interleaved noise + multiple JSON frames → last valid frame wins
        let stdout = r#"Starting process...
{"frame": 1, "status": "initializing"}
Some debug output
Warning: something happened
{"frame": 2, "status": "processing"}
More noise here
{"frame": 3, "status": "complete"}
Done!"#;
        let result = parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(json) => {
                let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
                assert_eq!(parsed["frame"], 3);
                assert_eq!(parsed["status"], "complete");
            }
            NdjsonResult::NoValidJson { .. } => panic!("Expected ValidJson"),
        }
    }

    #[test]
    fn test_parse_ndjson_no_valid_json() {
        let stdout = "This is just plain text\nNo JSON here\nJust noise";
        let result = parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(_) => panic!("Expected NoValidJson"),
            NdjsonResult::NoValidJson { tail_excerpt } => {
                assert_eq!(tail_excerpt, stdout);
            }
        }
    }

    #[test]
    fn test_parse_ndjson_partial_json() {
        // AT-RUN-005: Partial JSON followed by timeout → claude_failure with excerpt
        let stdout = r#"{"frame": 1, "status": "ok"}
{"frame": 2, "incomplete": tru"#;
        let result = parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(json) => {
                // Should return the last valid JSON (frame 1)
                let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
                assert_eq!(parsed["frame"], 1);
                assert_eq!(parsed["status"], "ok");
            }
            NdjsonResult::NoValidJson { .. } => panic!("Expected ValidJson from first frame"),
        }
    }

    #[test]
    fn test_parse_ndjson_only_partial_json() {
        let stdout = r#"{"incomplete": tru"#;
        let result = parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(_) => panic!("Expected NoValidJson"),
            NdjsonResult::NoValidJson { tail_excerpt } => {
                assert_eq!(tail_excerpt, stdout);
            }
        }
    }

    #[test]
    fn test_parse_ndjson_empty_string() {
        let stdout = "";
        let result = parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(_) => panic!("Expected NoValidJson"),
            NdjsonResult::NoValidJson { tail_excerpt } => {
                assert_eq!(tail_excerpt, "");
            }
        }
    }

    #[test]
    fn test_parse_ndjson_only_whitespace() {
        let stdout = "   \n\n  \t  \n";
        let result = parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(_) => panic!("Expected NoValidJson"),
            NdjsonResult::NoValidJson { tail_excerpt } => {
                assert_eq!(tail_excerpt, stdout);
            }
        }
    }

    #[test]
    fn test_parse_ndjson_tail_excerpt_truncation() {
        // Create a string longer than 256 characters
        let long_text = "x".repeat(300);
        let result = parse_ndjson(&long_text);

        match result {
            NdjsonResult::ValidJson(_) => panic!("Expected NoValidJson"),
            NdjsonResult::NoValidJson { tail_excerpt } => {
                assert_eq!(tail_excerpt.len(), 256);
                // Should be the last 256 characters
                assert_eq!(tail_excerpt, "x".repeat(256));
            }
        }
    }

    #[test]
    fn test_parse_ndjson_tail_excerpt_no_truncation() {
        let short_text = "Short text";
        let result = parse_ndjson(short_text);

        match result {
            NdjsonResult::ValidJson(_) => panic!("Expected NoValidJson"),
            NdjsonResult::NoValidJson { tail_excerpt } => {
                assert_eq!(tail_excerpt, short_text);
            }
        }
    }

    #[test]
    fn test_parse_ndjson_malformed_json_lines() {
        let stdout = r#"{"valid": "json"}
{malformed json}
{"another": "valid"}
[not an object]
{"final": "valid"}"#;
        let result = parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(json) => {
                let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
                assert_eq!(parsed["final"], "valid");
            }
            NdjsonResult::NoValidJson { .. } => panic!("Expected ValidJson"),
        }
    }

    #[test]
    fn test_parse_ndjson_json_array_is_valid() {
        // Arrays are valid JSON, should be accepted
        let stdout = r#"[1, 2, 3]
{"object": "value"}
[4, 5, 6]"#;
        let result = parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(json) => {
                let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
                assert!(parsed.is_array());
                assert_eq!(parsed[0], 4);
            }
            NdjsonResult::NoValidJson { .. } => panic!("Expected ValidJson"),
        }
    }

    #[test]
    fn test_parse_ndjson_json_primitives() {
        // JSON primitives (strings, numbers, booleans, null) are valid JSON
        let stdout = r#""string value"
42
true
null
{"final": "object"}"#;
        let result = parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(json) => {
                let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
                assert_eq!(parsed["final"], "object");
            }
            NdjsonResult::NoValidJson { .. } => panic!("Expected ValidJson"),
        }
    }

    #[test]
    fn test_parse_ndjson_unicode_content() {
        let stdout = r#"{"message": "Hello 世界"}
{"emoji": "🎉🎊"}
{"final": "完成"}"#;
        let result = parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(json) => {
                let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
                assert_eq!(parsed["final"], "完成");
            }
            NdjsonResult::NoValidJson { .. } => panic!("Expected ValidJson"),
        }
    }

    #[test]
    fn test_parse_ndjson_escaped_characters() {
        let stdout = r#"{"path": "C:\\Users\\test\\file.txt"}
{"quote": "He said \\"hello\\""}
{"final": "done"}"#;
        let result = parse_ndjson(stdout);

        match result {
            NdjsonResult::ValidJson(json) => {
                let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
                assert_eq!(parsed["final"], "done");
            }
            NdjsonResult::NoValidJson { .. } => panic!("Expected ValidJson"),
        }
    }
}
