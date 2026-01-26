use color_eyre::Section;
use gray_matter::{Matter, engine::YAML};
use serde::Deserialize;

use crate::{
    config::SiteMeta,
    types::{IsoDate, Tag, Tags},
    utils::{escape_attr, escape_text},
};

#[derive(Deserialize, Default, Debug)]
pub struct Header {
    title: Option<String>,
    subtitle: Option<String>,
    description: Option<String>,
    canonical: Option<String>,
    #[serde(alias = "og_image", alias = "image")]
    image: Option<String>,
    #[serde(alias = "og_title")]
    og_title: Option<String>,
    #[serde(alias = "og_description")]
    og_description: Option<String>,
    #[serde(alias = "og_type")]
    og_type: Option<String>,
    twitter_card: Option<String>,
    twitter_creator: Option<String>,
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

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
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

        if let Some(canonical) = self.canonical.as_ref() {
            result.push_str(&format!(
                r#"
<link rel="canonical" href="{}">"#,
                escape_attr(canonical)
            ));
        }

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

    /// Render OpenGraph + Twitter meta tags for an article page.
    pub fn opengraph_meta(&self, page_url: &str, site: &SiteMeta) -> String {
        let title = self
            .og_title
            .as_deref()
            .or(self.title.as_deref())
            .unwrap_or(site.title.as_str());
        let description = self
            .og_description
            .as_deref()
            .or(self.description.as_deref())
            .unwrap_or(site.description.as_str());
        let url = self.canonical.as_deref().unwrap_or(page_url);
        let og_type = self.og_type.as_deref().unwrap_or("article");
        let twitter_card = self
            .twitter_card
            .as_deref()
            .unwrap_or("summary_large_image");
        let twitter_creator = self
            .twitter_creator
            .as_deref()
            .or(Some(site.author.as_str()));

        let image_url = self
            .image
            .as_deref()
            .or(site.default_image.as_deref())
            .map(|img| absolute_url(&site.base_url, img));

        render_social_meta(
            title,
            description,
            url,
            og_type,
            twitter_card,
            twitter_creator,
            image_url.as_deref(),
        )
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

/// Render OpenGraph + Twitter meta tags for non-article pages (e.g., index, tag listings).
pub fn generic_og_meta(
    page_title: &str,
    page_description: &str,
    page_url: &str,
    site: &SiteMeta,
    image_override: Option<&str>,
) -> String {
    let image_url = image_override
        .or(site.default_image.as_deref())
        .map(|img| absolute_url(&site.base_url, img));

    render_social_meta(
        page_title,
        page_description,
        page_url,
        "website",
        "summary_large_image",
        Some(site.author.as_str()),
        image_url.as_deref(),
    )
}

fn absolute_url(base: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        path.to_string()
    } else {
        let base = base.trim_end_matches('/');
        let path = path.trim_start_matches('/');
        format!("{base}/{path}")
    }
}

fn render_social_meta(
    title: &str,
    description: &str,
    url: &str,
    og_type: &str,
    twitter_card: &str,
    twitter_creator: Option<&str>,
    image_url: Option<&str>,
) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        r#"
<meta property="og:title" content="{}">"#,
        escape_attr(title)
    ));
    out.push_str(&format!(
        r#"
<meta property="og:description" content="{}">"#,
        escape_attr(description)
    ));
    out.push_str(&format!(
        r#"
<meta property="og:type" content="{}">"#,
        escape_attr(og_type)
    ));
    out.push_str(&format!(
        r#"
<meta property="og:url" content="{}">"#,
        escape_attr(url)
    ));
    if let Some(img) = image_url {
        out.push_str(&format!(
            r#"
<meta property="og:image" content="{}">"#,
            escape_attr(img)
        ));
        out.push_str(&format!(
            r#"
<meta name="twitter:image" content="{}">"#,
            escape_attr(img)
        ));
    }
    out.push_str(&format!(
        r#"
<meta name="twitter:card" content="{}">"#,
        escape_attr(twitter_card)
    ));
    out.push_str(&format!(
        r#"
<meta name="twitter:title" content="{}">"#,
        escape_attr(title)
    ));
    out.push_str(&format!(
        r#"
<meta name="twitter:description" content="{}">"#,
        escape_attr(description)
    ));
    if let Some(creator) = twitter_creator {
        out.push_str(&format!(
            r#"
<meta name="twitter:creator" content="{}">"#,
            escape_attr(creator)
        ));
    }
    out.push_str(&format!(
        r#"
<link rel="canonical" href="{}">"#,
        escape_attr(url)
    ));
    out
}
