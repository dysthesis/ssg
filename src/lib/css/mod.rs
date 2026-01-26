use std::{fs, path::Path};

use crate::transformer::code_block::highlight_css;

pub fn build_css(css_path: &Path) -> color_eyre::Result<String> {
    let mut raw = fs::read_to_string(css_path)?;
    raw.push('\n');
    raw.push_str(highlight_css());
    Ok(raw)
}
