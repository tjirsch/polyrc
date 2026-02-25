use std::path::Path;
use crate::error::Result;
use crate::ir::Rule;

/// Reads a tool-specific configuration location and produces a list of Rules.
/// `path` is the project root directory (or user home for user-scope formats).
pub trait Parser {
    fn parse(&self, path: &Path) -> Result<Vec<Rule>>;
}
