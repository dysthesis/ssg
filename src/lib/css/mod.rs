use std::{fs, path::Path};

pub fn build_css(css_path: &Path) -> color_eyre::Result<String> {
    fs::read_to_string(css_path).map_err(Into::into)
}
