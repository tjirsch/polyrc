use std::path::Path;
use crate::error::Result;
use crate::ir::Rule;

/// Writes a list of Rules to the tool-specific configuration location.
/// `target` is the project root directory to write into.
pub trait Writer {
    fn write(&self, rules: &[Rule], target: &Path) -> Result<()>;
}
