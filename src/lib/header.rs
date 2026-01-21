use color_eyre::Section;
use gray_matter::{Matter, engine::YAML};
use serde::Deserialize;

#[derive(Deserialize, Default, Debug)]
pub struct Header {
    title: Option<String>,
    subtitle: Option<String>,
    description: Option<String>,
    ctime: Option<String>,
    mtime: Option<String>,
    tags: Option<Vec<String>>,
}

impl TryFrom<&str> for Header {
    type Error = color_eyre::Report;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let matter = Matter::<YAML>::new();
        matter
            .parse::<Header>(value)
            .with_note(|| "While parsing frontmatter.")
            .map(|res| res.data.unwrap_or_default())
    }
}

impl Header {
    pub fn to_html(&self) -> String {
        let mut result = String::new();

        let title = self
            .title
            .as_ref()
            .map(|title| {
                format!(
                    r#"
<title>
{}
</title>"#,
                    escape_text(title)
                )
            })
            .unwrap_or_default();

        let description = self
            .description
            .as_ref()
            .map(|desc| {
                format!(
                    r#"
<meta name="description" content="{}">"#,
                    escape_attr(desc)
                )
            })
            .unwrap_or_default();

        result.push_str(&title);
        result.push_str(&description);

        result.push_str(
            r#"
<link rel="stylesheet" href="style.css">"#,
        );

        result.push_str(
            r#"
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.27/dist/katex.css" integrity="sha384-m7LqaUc4JRc2uA7D4zSVUs/sgkYhmOOe9+Gd8DFmmAXH8vzs15fmw05YXvpxsoQB" crossorigin="anonymous">"#,
        );

        result
    }

    pub fn generate_body_head(&self) -> String {
        let mut result = String::new();

        let title = self
            .title
            .as_ref()
            .map(|title| {
                format!(
                    r#"<h1>{}</h1>
"#,
                    escape_text(title)
                )
            })
            .unwrap_or_default();

        let subtitle = self
            .subtitle
            .as_ref()
            .map(|sub| {
                format!(
                    r#"<p class="subtitle">{}</p>
"#,
                    escape_text(sub)
                )
            })
            .unwrap_or_default();

        let meta = self.render_body_meta();

        result.push_str(&title);
        result.push_str(&subtitle);
        result.push_str(&meta);

        result
    }

    fn render_body_meta(&self) -> String {
        let has_any = self.ctime.is_some()
            || self.mtime.is_some()
            || self.tags.as_ref().is_some_and(|t| !t.is_empty());

        if !has_any {
            return String::new();
        }

        let mut parts: Vec<String> = Vec::new();

        if let Some(ctime) = self.ctime.as_ref() {
            parts.push(format!(
                r#"<span class="meta-item">Created: <time datetime="{0}">{0}</time></span>"#,
                escape_attr(ctime)
            ));
        }

        if let Some(mtime) = self.mtime.as_ref() {
            parts.push(format!(
                r#"<span class="meta-item">Updated: <time datetime="{0}">{0}</time></span>"#,
                escape_attr(mtime)
            ));
        }

        if let Some(tags) = self.tags.as_ref().filter(|t| !t.is_empty()) {
            let rendered_tags = tags
                .iter()
                .map(|t| format!(r#"<span class="tag">{}</span>"#, escape_text(t)))
                .collect::<Vec<_>>()
                .join(" ");

            parts.push(format!(
                r#"<span class="meta-item">Tags: {}</span>"#,
                rendered_tags
            ));
        }

        format!(
            r#"<p class="meta">{}</p>
"#,
            parts.join(r#"<span class="meta-sep">·</span>"#)
        )
    }
}

fn escape_text(s: &str) -> String {
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

fn escape_attr(s: &str) -> String {
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
