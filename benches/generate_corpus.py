#!/usr/bin/env python3
"""Generate benchmark corpus files"""

import os
import hashlib
import json
from pathlib import Path

CORPUS_DIR = Path("benches/corpora")

def generate_plain_corpus():
    """Generate plain text corpus files"""
    plain_dir = CORPUS_DIR / "plain"

    # Read the 1k base file
    with open(plain_dir / "1k.md", "r") as f:
        base_content = f.read()

    # Generate 8k
    with open(plain_dir / "8k.md", "w") as f:
        while f.tell() < 8192:
            f.write(base_content)
            f.write("\n\n")

    # Generate 64k
    with open(plain_dir / "64k.md", "w") as f:
        while f.tell() < 65536:
            f.write(base_content)
            f.write("\n\n")

    # Generate 1m
    with open(plain_dir / "1m.md", "w") as f:
        while f.tell() < 1048576:
            f.write(base_content)
            f.write("\n\n")

    print("Generated plain corpus files")

def generate_code_dense_corpus():
    """Generate code-dense corpus files"""
    code_dir = CORPUS_DIR / "code_dense"

    # Language samples
    rust_code = '''fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}'''

    python_code = '''def quick_sort(arr):
    if len(arr) <= 1:
        return arr
    pivot = arr[len(arr) // 2]
    left = [x for x in arr if x < pivot]
    middle = [x for x in arr if x == pivot]
    right = [x for x in arr if x > pivot]
    return quick_sort(left) + middle + quick_sort(right)'''

    js_code = '''function debounce(func, wait) {
    let timeout;
    return function executedFunction(...args) {
        const later = () => {
            clearTimeout(timeout);
            func(...args);
        };
        clearTimeout(timeout);
        timeout = setTimeout(later, wait);
    };
}'''

    unknown_lang = '''UNKNOWN_SYNTAX ::= { special tokens }
    WEIRD_CONSTRUCT |> process()
    @decorator[complex]'''

    adversarial_lang = '''```<script>alert('xss')</script>
    "; DROP TABLE code;--
    ../../../etc/passwd'''

    def make_code_block(lang, code):
        return f"```{lang}\n{code}\n```\n\n"

    # 8k with 5 blocks
    with open(code_dir / "8k_5blocks.md", "w") as f:
        f.write("# Code Dense Document - Small\n\n")
        f.write("This document contains multiple code blocks.\n\n")
        for i in range(5):
            if i % 5 == 0:
                f.write(make_code_block("rust", rust_code))
            elif i % 5 == 1:
                f.write(make_code_block("python", python_code))
            elif i % 5 == 2:
                f.write(make_code_block("javascript", js_code))
            elif i % 5 == 3:
                f.write(make_code_block("unknown", unknown_lang))
            else:
                f.write(make_code_block("<>invalid", adversarial_lang))
            f.write("Text between code blocks.\n\n")
        # Pad to 8k
        while f.tell() < 8192:
            f.write("Padding text to reach target size. ")

    # 64k with 40 blocks
    with open(code_dir / "64k_40blocks.md", "w") as f:
        f.write("# Code Dense Document - Medium\n\n")
        for i in range(40):
            if i % 5 == 0:
                f.write(make_code_block("rust", rust_code))
            elif i % 5 == 1:
                f.write(make_code_block("python", python_code))
            elif i % 5 == 2:
                f.write(make_code_block("javascript", js_code))
            elif i % 5 == 3:
                f.write(make_code_block("unknown", unknown_lang))
            else:
                f.write(make_code_block("<>invalid", adversarial_lang))
            f.write("Text between blocks.\n\n")
        while f.tell() < 65536:
            f.write("Padding text. ")

    # 1m with 200 blocks
    with open(code_dir / "1m_200blocks.md", "w") as f:
        f.write("# Code Dense Document - Large\n\n")
        for i in range(200):
            if i % 5 == 0:
                f.write(make_code_block("rust", rust_code))
            elif i % 5 == 1:
                f.write(make_code_block("python", python_code))
            elif i % 5 == 2:
                f.write(make_code_block("javascript", js_code))
            elif i % 5 == 3:
                f.write(make_code_block("unknown", unknown_lang))
            else:
                f.write(make_code_block("<>invalid", adversarial_lang))
            f.write("Text between blocks.\n\n")
        while f.tell() < 1048576:
            f.write("Padding text. ")

    print("Generated code_dense corpus files")

def generate_math_dense_corpus():
    """Generate math-dense corpus files"""
    math_dir = CORPUS_DIR / "math_dense"

    valid_inline = [
        "$x + y = z$",
        "$\\alpha + \\beta = \\gamma$",
        "$f(x) = x^2 + 2x + 1$",
        "$\\int_0^1 x dx = \\frac{1}{2}$",
    ]

    valid_display = [
        "$$\\sum_{i=1}^n i = \\frac{n(n+1)}{2}$$",
        "$$E = mc^2$$",
        "$$\\nabla \\cdot \\mathbf{E} = \\frac{\\rho}{\\epsilon_0}$$",
        "$$\\frac{d}{dx} e^x = e^x$$",
    ]

    invalid_math = [
        "$\\invalid{syntax}$",
        "$\\",
        "$$unclosed",
        "$}{$",
    ]

    # 8k valid
    with open(math_dir / "8k_valid.md", "w") as f:
        f.write("# Math Dense Document - Small Valid\n\n")
        i = 0
        while f.tell() < 8192:
            f.write(f"Inline math {valid_inline[i % len(valid_inline)]} in text. ")
            if i % 3 == 0:
                f.write(f"\n\n{valid_display[i % len(valid_display)]}\n\n")
            i += 1

    # 64k valid
    with open(math_dir / "64k_valid.md", "w") as f:
        f.write("# Math Dense Document - Medium Valid\n\n")
        i = 0
        while f.tell() < 65536:
            f.write(f"Inline {valid_inline[i % len(valid_inline)]} text. ")
            if i % 3 == 0:
                f.write(f"\n\n{valid_display[i % len(valid_display)]}\n\n")
            i += 1

    # 64k mixed valid/invalid (80% valid, 20% invalid)
    with open(math_dir / "64k_mixed_valid_invalid.md", "w") as f:
        f.write("# Math Dense Document - Medium Mixed\n\n")
        i = 0
        while f.tell() < 65536:
            if i % 5 == 0:
                f.write(f"Invalid {invalid_math[i % len(invalid_math)]} math. ")
            else:
                f.write(f"Valid {valid_inline[i % len(valid_inline)]} math. ")
            if i % 3 == 0:
                if i % 5 == 0:
                    f.write(f"\n\nInvalid display math\n\n")
                else:
                    f.write(f"\n\n{valid_display[i % len(valid_display)]}\n\n")
            i += 1

    # 1m mixed
    with open(math_dir / "1m_mixed_valid_invalid.md", "w") as f:
        f.write("# Math Dense Document - Large Mixed\n\n")
        i = 0
        while f.tell() < 1048576:
            if i % 5 == 0:
                f.write(f"Invalid {invalid_math[i % len(invalid_math)]} math. ")
            else:
                f.write(f"Valid {valid_inline[i % len(valid_inline)]} math. ")
            if i % 3 == 0:
                if i % 5 == 0:
                    f.write(f"\n\nInvalid display\n\n")
                else:
                    f.write(f"\n\n{valid_display[i % len(valid_display)]}\n\n")
            i += 1

    print("Generated math_dense corpus files")

def generate_mixed_features_corpus():
    """Generate mixed-features corpus files"""
    mixed_dir = CORPUS_DIR / "mixed_features"

    features = [
        "# Heading\n\nParagraph text.\n\n",
        "|Column 1|Column 2|\n|--------|--------|\n|Data 1|Data 2|\n\n",
        "Inline math $x^2$ in text.\n\n",
        "$$\n\\sum_{i=1}^n i\n$$\n\n",
        "```rust\nfn main() {\n    println!(\"Hello\");\n}\n```\n\n",
        "This has ~~strikethrough~~ text.\n\n",
        "Footnote reference[^1].\n\n[^1]: Footnote content.\n\n",
        "---\ntitle: Metadata\nauthor: Test\n---\n\n",
    ]

    for size, filename in [(8192, "8k.md"), (65536, "64k.md"), (1048576, "1m.md")]:
        with open(mixed_dir / filename, "w") as f:
            f.write(f"# Mixed Features Document\n\n")
            i = 0
            while f.tell() < size:
                f.write(features[i % len(features)])
                i += 1

    print("Generated mixed_features corpus files")

def generate_adversarial_corpus():
    """Generate adversarial corpus files"""
    adv_dir = CORPUS_DIR / "adversarial"

    # Escape-dense
    escape_text = '&lt;&gt;&quot;&#x27;&amp;' * 50 + '\n'
    with open(adv_dir / "64k_escape_dense.md", "w") as f:
        while f.tell() < 65536:
            f.write(escape_text)

    with open(adv_dir / "1m_escape_dense.md", "w") as f:
        while f.tell() < 1048576:
            f.write(escape_text)

    # Long lines
    long_line = "a" * 10000 + "\n"
    with open(adv_dir / "1m_long_lines.md", "w") as f:
        while f.tell() < 1048576:
            f.write(long_line)

    # Nested constructs
    with open(adv_dir / "1m_nested_constructs.md", "w") as f:
        nesting_depth = 50
        while f.tell() < 1048576:
            for _ in range(nesting_depth):
                f.write("> ")
            f.write("Deeply nested blockquote\n\n")
            f.write("* " * 10 + "Nested list\n\n")

    print("Generated adversarial corpus files")

def generate_snippets():
    """Generate snippet files for micro benchmarks"""
    snippets_dir = CORPUS_DIR / "snippets"

    # Code snippets
    with open(snippets_dir / "code_rust_small.txt", "w") as f:
        f.write("fn main() {\n    println!(\"Hello, world!\");\n}\n")

    with open(snippets_dir / "code_rust_large.txt", "w") as f:
        rust_large = '''use std::collections::HashMap;
use std::io::{self, Read};

fn main() -> io::Result<()> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;

    let mut word_counts = HashMap::new();
    for word in buffer.split_whitespace() {
        *word_counts.entry(word.to_lowercase()).or_insert(0) += 1;
    }

    let mut counts: Vec<_> = word_counts.iter().collect();
    counts.sort_by(|a, b| b.1.cmp(a.1));

    for (word, count) in counts.iter().take(10) {
        println!("{}: {}", word, count);
    }

    Ok(())
}
'''
        f.write(rust_large)

    with open(snippets_dir / "code_plain_large.txt", "w") as f:
        f.write("Plain text " * 100)

    # Math snippets
    with open(snippets_dir / "math_simple.tex", "w") as f:
        f.write("x + y = z")

    with open(snippets_dir / "math_complex.tex", "w") as f:
        f.write("\\int_0^\\infty e^{-x^2} dx = \\frac{\\sqrt{\\pi}}{2}")

    with open(snippets_dir / "math_invalid.tex", "w") as f:
        f.write("\\invalid{syntax}")

    # Stylesheet snippets
    with open(snippets_dir / "style_small.css", "w") as f:
        f.write("body { margin: 0; padding: 0; }\n")

    with open(snippets_dir / "style_large.css", "w") as f:
        css = '''body {
    margin: 0;
    padding: 20px;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    line-height: 1.6;
    color: #333;
}

h1, h2, h3, h4, h5, h6 {
    margin-top: 1.5em;
    margin-bottom: 0.5em;
    font-weight: 600;
}

code {
    background: #f4f4f4;
    padding: 2px 6px;
    border-radius: 3px;
}

pre {
    background: #f4f4f4;
    padding: 10px;
    border-radius: 5px;
    overflow-x: auto;
}
'''
        f.write(css)

    print("Generated snippet files")

if __name__ == "__main__":
    generate_plain_corpus()
    generate_code_dense_corpus()
    generate_math_dense_corpus()
    generate_mixed_features_corpus()
    generate_adversarial_corpus()
    generate_snippets()
    print("\nAll corpus files generated successfully!")
