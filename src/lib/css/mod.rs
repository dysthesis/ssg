use std::{fs, path::Path};

use color_eyre::eyre::eyre;
use lightningcss::{
    printer::PrinterOptions,
    stylesheet::{MinifyOptions, ParserOptions, StyleSheet},
};

pub fn build_css(css_path: &Path) -> color_eyre::Result<String> {
    let raw = fs::read_to_string(css_path)?;

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
