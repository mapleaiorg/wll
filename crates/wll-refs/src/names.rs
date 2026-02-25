//! Branch name validation following git-style conventions.
//!
//! Valid branch names:
//! - Must be non-empty
//! - Must not contain whitespace, `~`, `^`, `:`, `?`, `*`, `[`, `\`
//! - Must not contain `..` (double dot) or `@{`
//! - Must not start or end with `.` or `/`
//! - Must not end with `.lock`
//! - Must not contain consecutive slashes (`//`)
//! - Components between slashes must be non-empty

use crate::error::{RefError, Result};

/// Characters that are forbidden anywhere in a branch name.
const FORBIDDEN_CHARS: &[char] = &[' ', '\t', '\n', '\r', '~', '^', ':', '?', '*', '[', '\\'];

/// Validate a branch name, returning `Ok(())` if valid.
///
/// Follows git-style naming conventions to prevent ambiguity and filesystem
/// issues.
///
/// # Examples
///
/// ```
/// use wll_refs::names::validate_branch_name;
///
/// assert!(validate_branch_name("main").is_ok());
/// assert!(validate_branch_name("feature/auth").is_ok());
/// assert!(validate_branch_name("").is_err());
/// assert!(validate_branch_name("bad..name").is_err());
/// ```
pub fn validate_branch_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(RefError::InvalidBranchName {
            name: name.to_string(),
            reason: "branch name must not be empty".into(),
        });
    }

    // Check for forbidden characters.
    for ch in FORBIDDEN_CHARS {
        if name.contains(*ch) {
            return Err(RefError::InvalidBranchName {
                name: name.to_string(),
                reason: format!("contains forbidden character: {ch:?}"),
            });
        }
    }

    // Must not contain `..` (parent traversal).
    if name.contains("..") {
        return Err(RefError::InvalidBranchName {
            name: name.to_string(),
            reason: "must not contain '..'".into(),
        });
    }

    // Must not contain `@{` (reflog syntax).
    if name.contains("@{") {
        return Err(RefError::InvalidBranchName {
            name: name.to_string(),
            reason: "must not contain '@{'".into(),
        });
    }

    // Must not start or end with `.`.
    if name.starts_with('.') || name.ends_with('.') {
        return Err(RefError::InvalidBranchName {
            name: name.to_string(),
            reason: "must not start or end with '.'".into(),
        });
    }

    // Must not start or end with `/`.
    if name.starts_with('/') || name.ends_with('/') {
        return Err(RefError::InvalidBranchName {
            name: name.to_string(),
            reason: "must not start or end with '/'".into(),
        });
    }

    // Must not end with `.lock`.
    if name.ends_with(".lock") {
        return Err(RefError::InvalidBranchName {
            name: name.to_string(),
            reason: "must not end with '.lock'".into(),
        });
    }

    // Must not contain consecutive slashes.
    if name.contains("//") {
        return Err(RefError::InvalidBranchName {
            name: name.to_string(),
            reason: "must not contain consecutive slashes '//'".into(),
        });
    }

    // Path components between slashes must be non-empty and not start with `.`.
    for component in name.split('/') {
        if component.is_empty() {
            return Err(RefError::InvalidBranchName {
                name: name.to_string(),
                reason: "path components must not be empty".into(),
            });
        }
        if component.starts_with('.') {
            return Err(RefError::InvalidBranchName {
                name: name.to_string(),
                reason: format!("component must not start with '.': {component:?}"),
            });
        }
    }

    Ok(())
}

/// Validate a tag name. Same rules as branch names.
pub fn validate_tag_name(name: &str) -> Result<()> {
    validate_branch_name(name).map_err(|_| RefError::InvalidBranchName {
        name: name.to_string(),
        reason: "invalid tag name".into(),
    })
}

/// Validate a remote name. Must be a simple identifier (no slashes).
pub fn validate_remote_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(RefError::InvalidBranchName {
            name: name.to_string(),
            reason: "remote name must not be empty".into(),
        });
    }
    if name.contains('/') {
        return Err(RefError::InvalidBranchName {
            name: name.to_string(),
            reason: "remote name must not contain '/'".into(),
        });
    }
    for ch in FORBIDDEN_CHARS {
        if name.contains(*ch) {
            return Err(RefError::InvalidBranchName {
                name: name.to_string(),
                reason: format!("remote name contains forbidden character: {ch:?}"),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_simple_names() {
        assert!(validate_branch_name("main").is_ok());
        assert!(validate_branch_name("develop").is_ok());
        assert!(validate_branch_name("my-branch").is_ok());
        assert!(validate_branch_name("v1.0").is_ok());
    }

    #[test]
    fn valid_nested_names() {
        assert!(validate_branch_name("feature/auth").is_ok());
        assert!(validate_branch_name("feature/deep/nested/branch").is_ok());
        assert!(validate_branch_name("user/alice/fix-123").is_ok());
    }

    #[test]
    fn reject_empty_name() {
        assert!(validate_branch_name("").is_err());
    }

    #[test]
    fn reject_double_dot() {
        assert!(validate_branch_name("bad..name").is_err());
        assert!(validate_branch_name("a..b").is_err());
    }

    #[test]
    fn reject_whitespace() {
        assert!(validate_branch_name("has space").is_err());
        assert!(validate_branch_name("has\ttab").is_err());
        assert!(validate_branch_name("has\nnewline").is_err());
    }

    #[test]
    fn reject_forbidden_chars() {
        assert!(validate_branch_name("a~b").is_err());
        assert!(validate_branch_name("a^b").is_err());
        assert!(validate_branch_name("a:b").is_err());
        assert!(validate_branch_name("a?b").is_err());
        assert!(validate_branch_name("a*b").is_err());
        assert!(validate_branch_name("a[b").is_err());
        assert!(validate_branch_name("a\\b").is_err());
    }

    #[test]
    fn reject_dot_boundaries() {
        assert!(validate_branch_name(".hidden").is_err());
        assert!(validate_branch_name("trailing.").is_err());
    }

    #[test]
    fn reject_slash_boundaries() {
        assert!(validate_branch_name("/leading").is_err());
        assert!(validate_branch_name("trailing/").is_err());
    }

    #[test]
    fn reject_consecutive_slashes() {
        assert!(validate_branch_name("a//b").is_err());
    }

    #[test]
    fn reject_lock_suffix() {
        assert!(validate_branch_name("main.lock").is_err());
    }

    #[test]
    fn reject_at_brace() {
        assert!(validate_branch_name("ref@{0}").is_err());
    }

    #[test]
    fn reject_component_starting_with_dot() {
        assert!(validate_branch_name("feature/.hidden").is_err());
    }
}
