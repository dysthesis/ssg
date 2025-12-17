//! Cold start helper binary for benchmarking first-time initialization costs
//!
//! This binary is spawned in a fresh process to measure cold-start performance.
//! It performs exactly one operation and exits.

use libssg::highlighter::{CodeblockHighlighter, syntect::SyntectHighlighter};
use libssg::math::{MathRenderer, katex::KatexRenderer};
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: cold_start_helper <operation> [args...]");
        eprintln!("Operations:");
        eprintln!("  noop - Do nothing (baseline for overhead measurement)");
        eprintln!("  syntect <snippet> <language> - Highlight code snippet");
        eprintln!("  katex <snippet> <display_mode> - Render math snippet");
        std::process::exit(1);
    }

    let operation = &args[1];

    match operation.as_str() {
        "noop" => {
            // Do nothing - this measures process spawn + arg parse + exit overhead
        }
        "syntect" => {
            if args.len() < 4 {
                eprintln!("Usage: cold_start_helper syntect <snippet> <language>");
                std::process::exit(1);
            }

            // Take snippet directly from command line to avoid filesystem overhead
            let source = &args[2];
            let language = &args[3];

            let highlighter = SyntectHighlighter::default();
            let language_opt = if language == "none" {
                None
            } else {
                Some(language.as_str())
            };

            let _result = highlighter.render_codeblock(source, language_opt);
        }
        "katex" => {
            if args.len() < 4 {
                eprintln!("Usage: cold_start_helper katex <snippet> <display_mode>");
                std::process::exit(1);
            }

            // Take snippet directly from command line to avoid filesystem overhead
            let source = &args[2];
            let display_mode = args[3] == "true";

            let renderer = KatexRenderer::new();
            let _result = renderer.render_math(source, display_mode);
        }
        _ => {
            eprintln!("Unknown operation: {}", operation);
            eprintln!("Valid operations: noop, syntect, katex");
            std::process::exit(1);
        }
    }
}
