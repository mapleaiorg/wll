//! Blob-level diff: line-by-line comparison of file contents.
//!
//! Uses the `similar` crate (Myers diff algorithm) to produce structured
//! hunks with context lines.

use similar::{ChangeTag, TextDiff};

/// The result of diffing two blobs (file contents).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlobDiff {
    /// The diff hunks.
    pub hunks: Vec<DiffHunk>,
    /// Total number of lines in the old content.
    pub old_lines: usize,
    /// Total number of lines in the new content.
    pub new_lines: usize,
}

impl BlobDiff {
    /// Returns `true` if the two blobs are identical.
    pub fn is_empty(&self) -> bool {
        self.hunks.is_empty()
    }

    /// Total number of lines added across all hunks.
    pub fn additions(&self) -> usize {
        self.hunks
            .iter()
            .flat_map(|h| &h.lines)
            .filter(|l| matches!(l, DiffLine::Added(_)))
            .count()
    }

    /// Total number of lines removed across all hunks.
    pub fn deletions(&self) -> usize {
        self.hunks
            .iter()
            .flat_map(|h| &h.lines)
            .filter(|l| matches!(l, DiffLine::Removed(_)))
            .count()
    }
}

/// A contiguous region of changes in a diff.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffHunk {
    /// Line number in the old content where this hunk starts (1-based).
    pub old_start: usize,
    /// Number of lines from the old content in this hunk.
    pub old_count: usize,
    /// Line number in the new content where this hunk starts (1-based).
    pub new_start: usize,
    /// Number of lines from the new content in this hunk.
    pub new_count: usize,
    /// The individual diff lines in this hunk.
    pub lines: Vec<DiffLine>,
}

/// A single line in a diff hunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffLine {
    /// A line present in both old and new (context).
    Context(String),
    /// A line added in the new content.
    Added(String),
    /// A line removed from the old content.
    Removed(String),
}

/// Compute a line-by-line diff between two byte slices.
///
/// The content is interpreted as UTF-8 text. If the content is not valid
/// UTF-8 (binary files), a single hunk noting the binary difference is returned.
pub fn diff_blobs(old: &[u8], new: &[u8]) -> BlobDiff {
    // Handle binary content.
    let old_str = match std::str::from_utf8(old) {
        Ok(s) => s,
        Err(_) => {
            return make_binary_diff(old, new);
        }
    };
    let new_str = match std::str::from_utf8(new) {
        Ok(s) => s,
        Err(_) => {
            return make_binary_diff(old, new);
        }
    };

    // Identical content.
    if old_str == new_str {
        return BlobDiff {
            hunks: Vec::new(),
            old_lines: old_str.lines().count(),
            new_lines: new_str.lines().count(),
        };
    }

    let text_diff = TextDiff::from_lines(old_str, new_str);

    let old_lines = old_str.lines().count();
    let new_lines = new_str.lines().count();

    let mut hunks = Vec::new();

    for hunk in text_diff.grouped_ops(3) {
        let mut lines = Vec::new();
        let mut hunk_old_start = 0usize;
        let mut hunk_new_start = 0usize;
        let mut hunk_old_count = 0usize;
        let mut hunk_new_count = 0usize;
        let mut first = true;

        for op in &hunk {
            if first {
                hunk_old_start = op.old_range().start + 1;
                hunk_new_start = op.new_range().start + 1;
                first = false;
            }

            for change in text_diff.iter_changes(op) {
                let text = change.value().trim_end_matches('\n').to_string();
                match change.tag() {
                    ChangeTag::Equal => {
                        lines.push(DiffLine::Context(text));
                        hunk_old_count += 1;
                        hunk_new_count += 1;
                    }
                    ChangeTag::Delete => {
                        lines.push(DiffLine::Removed(text));
                        hunk_old_count += 1;
                    }
                    ChangeTag::Insert => {
                        lines.push(DiffLine::Added(text));
                        hunk_new_count += 1;
                    }
                }
            }
        }

        hunks.push(DiffHunk {
            old_start: hunk_old_start,
            old_count: hunk_old_count,
            new_start: hunk_new_start,
            new_count: hunk_new_count,
            lines,
        });
    }

    BlobDiff {
        hunks,
        old_lines,
        new_lines,
    }
}

/// Create a synthetic diff for binary content.
fn make_binary_diff(old: &[u8], new: &[u8]) -> BlobDiff {
    let mut lines = Vec::new();
    if !old.is_empty() {
        lines.push(DiffLine::Removed(format!("(binary content, {} bytes)", old.len())));
    }
    if !new.is_empty() {
        lines.push(DiffLine::Added(format!("(binary content, {} bytes)", new.len())));
    }

    BlobDiff {
        hunks: vec![DiffHunk {
            old_start: 1,
            old_count: if old.is_empty() { 0 } else { 1 },
            new_start: 1,
            new_count: if new.is_empty() { 0 } else { 1 },
            lines,
        }],
        old_lines: 0,
        new_lines: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_blobs_no_diff() {
        let content = b"hello\nworld\n";
        let diff = diff_blobs(content, content);
        assert!(diff.is_empty());
        assert_eq!(diff.additions(), 0);
        assert_eq!(diff.deletions(), 0);
    }

    #[test]
    fn single_line_addition() {
        let old = b"line1\nline2\n";
        let new = b"line1\nline2\nline3\n";

        let diff = diff_blobs(old, new);
        assert!(!diff.is_empty());
        assert_eq!(diff.additions(), 1);
        assert_eq!(diff.deletions(), 0);
    }

    #[test]
    fn single_line_deletion() {
        let old = b"line1\nline2\nline3\n";
        let new = b"line1\nline3\n";

        let diff = diff_blobs(old, new);
        assert!(!diff.is_empty());
        assert!(diff.deletions() >= 1);
    }

    #[test]
    fn modification_shows_remove_and_add() {
        let old = b"hello world\n";
        let new = b"hello universe\n";

        let diff = diff_blobs(old, new);
        assert!(!diff.is_empty());
        assert!(diff.additions() >= 1);
        assert!(diff.deletions() >= 1);
    }

    #[test]
    fn empty_to_content() {
        let diff = diff_blobs(b"", b"new content\n");
        assert!(!diff.is_empty());
        assert!(diff.additions() >= 1);
    }

    #[test]
    fn content_to_empty() {
        let diff = diff_blobs(b"old content\n", b"");
        assert!(!diff.is_empty());
        assert!(diff.deletions() >= 1);
    }

    #[test]
    fn binary_content_detection() {
        let old = &[0u8, 1, 2, 3, 0xFF, 0xFE];
        let new = &[4u8, 5, 6, 0xFF, 0xFE, 0xFD];

        let diff = diff_blobs(old, new);
        assert!(!diff.is_empty());
        // Binary diff produces synthetic lines.
        assert_eq!(diff.hunks.len(), 1);
    }

    #[test]
    fn hunk_line_numbers() {
        let old = b"a\nb\nc\nd\ne\n";
        let new = b"a\nb\nX\nd\ne\n";

        let diff = diff_blobs(old, new);
        assert!(!diff.is_empty());
        // The hunk should cover the area around line 3.
        let hunk = &diff.hunks[0];
        assert!(hunk.old_start >= 1);
        assert!(hunk.new_start >= 1);
    }

    #[test]
    fn multiline_diff() {
        let old = b"line1\nline2\nline3\nline4\nline5\n";
        let new = b"line1\nmodified\nline3\nnew_line\nline5\n";

        let diff = diff_blobs(old, new);
        assert!(!diff.is_empty());
        assert_eq!(diff.old_lines, 5);
        assert_eq!(diff.new_lines, 5);
    }

    #[test]
    fn context_lines_present() {
        let old = b"a\nb\nc\nd\ne\nf\ng\nh\ni\nj\n";
        let new = b"a\nb\nc\nd\nX\nf\ng\nh\ni\nj\n";

        let diff = diff_blobs(old, new);
        assert!(!diff.is_empty());

        // Should have context lines around the change.
        let hunk = &diff.hunks[0];
        let has_context = hunk.lines.iter().any(|l| matches!(l, DiffLine::Context(_)));
        assert!(has_context, "hunk should contain context lines");
    }
}
