use color_eyre::Section;
use gray_matter::{Matter, engine::YAML};
use serde::Deserialize;

use crate::{
    types::{IsoDate, Tag, Tags},
    utils::{escape_attr, escape_text},
};

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
    pub fn ctime(&self) -> Option<IsoDate> {
        self.ctime.as_deref().and_then(IsoDate::parse)
    }

    pub fn mtime(&self) -> Option<IsoDate> {
        self.mtime.as_deref().and_then(IsoDate::parse)
    }

    pub fn tags(&self) -> Tags {
        let parsed = self
            .tags
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter_map(|t| Tag::parse(t))
            .collect();
        Tags::new(parsed)
    }

    pub fn to_html(&self, css_href: &str, has_math: bool, katex_href: &str) -> String {
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

        if has_math {
            result.push_str(&format!(
                r#"
<link rel="stylesheet" href="{katex_href}">"#
            ));
        }

        result.push_str(&format!(
            r#"
<link rel="stylesheet" href="{}">"#,
            escape_attr(css_href),
        ));

        result
    }

    pub fn generate_body_head(&self, href_prefix: &str) -> String {
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
        let index_link = format!(
            r#"<p class="meta"><a href="{0}index.html">Index</a></p>
"#,
            escape_attr(href_prefix)
        );

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

        let meta = self.render_body_meta(href_prefix);
        result.push_str(&title);
        result.push_str(&subtitle);
        result.push_str(&index_link);
        result.push_str(&meta);

        result
    }

    fn render_body_meta(&self, href_prefix: &str) -> String {
        let has_any = self.ctime.is_some() || self.mtime.is_some() || !self.tags().is_empty();

        if !has_any {
            return String::new();
        }

        let mut parts: Vec<String> = Vec::new();

        if let Some(ctime) = self.ctime() {
            let ctime = ctime.as_str();
            parts.push(format!(
                r#"<span class="meta-item">Created: <time datetime="{0}">{0}</time></span>"#,
                escape_attr(ctime.as_str())
            ));
        }

        if let Some(mtime) = self.mtime() {
            let mtime = mtime.as_str();
            parts.push(format!(
                r#"<span class="meta-item">Updated: <time datetime="{0}">{0}</time></span>"#,
                escape_attr(mtime.as_str())
            ));
        }

        if !self.tags().is_empty() {
            let rendered_tags = self
                .tags()
                .0
                .iter()
                .map(|t| {
                    let href = format!(r#"{href_prefix}tags/{t}.html"#);
                    format!(
                        r#"<a class="tag" href="{}">{}</a>"#,
                        escape_attr(&href),
                        escape_text(t.as_str())
                    )
                })
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
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
}
