//! Cold start helper binary for benchmarking first-time initialization costs
//!
//! This binary is spawned in a fresh process to measure cold-start performance.
//! It performs exactly one operation and exits.

use libssg::renderer::katex::KatexRenderer;
use libssg::renderer::syntect::SyntectHighlighter;
use libssg::renderer::{CodeblockHighlighter, MathRenderer};
use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: cold_start_helper <operation> [args...]");
        eprintln!("Operations:");
        eprintln!("  syntect <file> <language> - Highlight code from file");
        eprintln!("  katex <file> <display_mode> - Render math from file");
        std::process::exit(1);
    }

    let operation = &args[1];

    match operation.as_str() {
        "syntect" => {
            if args.len() < 4 {
                eprintln!("Usage: cold_start_helper syntect <file> <language>");
                std::process::exit(1);
            }

            let file_path = &args[2];
            let language = &args[3];

            let source = fs::read_to_string(file_path)
                .unwrap_or_else(|e| panic!("Failed to read file {}: {}", file_path, e));

            let highlighter = SyntectHighlighter::default();
            let language_opt = if language == "none" {
                None
            } else {
                Some(language.as_str())
            };

            let _result = highlighter.render_codeblock(&source, language_opt);
        }
        "katex" => {
            if args.len() < 4 {
                eprintln!("Usage: cold_start_helper katex <file> <display_mode>");
                std::process::exit(1);
            }

            let file_path = &args[2];
            let display_mode = args[3] == "true";

            let source = fs::read_to_string(file_path)
                .unwrap_or_else(|e| panic!("Failed to read file {}: {}", file_path, e));

            let renderer = KatexRenderer::new();
            let _result = renderer.render_math(&source, display_mode);
        }
        _ => {
            eprintln!("Unknown operation: {}", operation);
            eprintln!("Valid operations: syntect, katex");
            std::process::exit(1);
        }
    }
}
