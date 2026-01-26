use std::{fs, path::Path};

use crate::transformer::code_block::highlight_css;
use color_eyre::eyre::eyre;
use lightningcss::{
    printer::PrinterOptions,
    stylesheet::{MinifyOptions, ParserOptions, StyleSheet},
};

pub fn build_css(css_path: &Path) -> color_eyre::Result<String> {
    let mut raw = fs::read_to_string(css_path)?;
    raw.push('\n');
    raw.push_str(highlight_css());

    let mut stylesheet = StyleSheet::parse(
        &raw,
        ParserOptions {
            filename: css_path.to_string_lossy().into_owned(),
            ..Default::default()
        },
    )
    .map_err(|e| eyre!(e.to_string()))?;

    stylesheet
        .minify(MinifyOptions::default())
        .map_err(|e| eyre!(e.to_string()))?;

    let res = stylesheet
        .to_css(PrinterOptions {
            minify: true,
            ..Default::default()
        })
        .map_err(|e| eyre!(e.to_string()))?;

    Ok(res.code)
}
