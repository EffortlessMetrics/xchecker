use super::parse::FixupParser;

impl FixupParser {
    /// Check if context lines match at a specific position
    pub(super) fn context_matches_at(
        &self,
        lines: &[String],
        pos: usize,
        context: &[&str],
    ) -> bool {
        if context.is_empty() {
            return true; // No context to match
        }

        // Check if we have enough lines
        if pos >= lines.len() {
            return false;
        }

        // Try to match context lines starting at pos
        let mut matches = 0;
        for (i, ctx_line) in context.iter().enumerate() {
            let file_pos = pos + i;
            if file_pos >= lines.len() {
                break;
            }
            if self.lines_match(&lines[file_pos], ctx_line) {
                matches += 1;
            }
        }

        // Require all context lines to match for exact match
        matches == context.len()
    }

    /// Find the best matching position for context within a search window
    pub(super) fn find_best_context_match(
        &self,
        lines: &[String],
        expected_pos: usize,
        context: &[&str],
        window: usize,
        min_ratio: f64,
    ) -> Option<(usize, f64)> {
        if context.is_empty() {
            return Some((expected_pos, 1.0));
        }

        let start = expected_pos.saturating_sub(window);
        let end = (expected_pos + window).min(lines.len());

        let mut best_match: Option<(usize, f64)> = None;

        for candidate in start..end {
            let score = self.context_match_score(lines, candidate, context);
            if score >= min_ratio && best_match.is_none_or(|(_, best_score)| score > best_score) {
                best_match = Some((candidate, score));
            }
        }

        best_match
    }

    /// Calculate match score for context at a position (0.0 to 1.0)
    fn context_match_score(&self, lines: &[String], pos: usize, context: &[&str]) -> f64 {
        if context.is_empty() {
            return 1.0;
        }

        let mut matches = 0;
        for (i, ctx_line) in context.iter().enumerate() {
            let file_pos = pos + i;
            if file_pos >= lines.len() {
                break;
            }
            if self.lines_match(&lines[file_pos], ctx_line) {
                matches += 1;
            }
        }

        (matches as f64) / (context.len() as f64)
    }

    /// Compare two lines with whitespace normalization
    fn lines_match(&self, file_line: &str, context_line: &str) -> bool {
        // Exact match
        if file_line == context_line {
            return true;
        }

        // Whitespace-normalized match (collapse multiple spaces, trim)
        let normalize = |s: &str| -> String { s.split_whitespace().collect::<Vec<_>>().join(" ") };

        normalize(file_line) == normalize(context_line)
    }
}
