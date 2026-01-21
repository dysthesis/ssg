use std::path::PathBuf;

use color_eyre::Section;
use gray_matter::{Matter, engine::YAML};
use serde::Deserialize;

#[derive(Deserialize, Default, Debug)]
pub struct FrontMatter {
    title: Option<String>,
    description: Option<String>,
    tags: Vec<String>,
}

impl TryFrom<&str> for FrontMatter {
    type Error = color_eyre::Report;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let matter = Matter::<YAML>::new();
        matter
            .parse::<FrontMatter>(value)
            .with_note(|| "While parsing frontmatter.")
            .map(|res| res.data.unwrap_or_else(FrontMatter::default))
    }
}

impl FrontMatter {
    pub fn to_html(self) -> String {
        let mut result = String::new();
        let title = self
            .title
            .map(|title| {
                format!(
                    r#"
<title>
{title}
</title>
        "#
                )
            })
            .unwrap_or_default();

        let description = self
            .description
            .map(|desc| {
                format!(
                    r#"<meta name="description" content="{}">"#,
                    escape_attr(&desc)
                )
            })
            .unwrap_or_default();

        result.push_str(&title);
        result.push_str(&description);
        result.push_str(r#"<link rel="stylesheet" href="style.css"">"#);
        result
    }
}
fn escape_attr(s: &str) -> String {
    // Minimal escaping suitable for attribute values and <title>.
    // You already have an HTML escaper in the code highlighting module; this keeps main.rs independent.
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(ch),
        }
    }
    out
}
