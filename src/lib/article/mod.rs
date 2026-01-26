use crate::{
    types::{Href, IsoDate, Tag},
    utils::{escape_attr, escape_text},
};

#[derive(Clone, Debug)]
pub struct Article {
    pub title: String,
    pub ctime: Option<IsoDate>,
    pub updated: Option<IsoDate>,
    pub summary: Option<String>,
    pub href: Href,
    pub tags: Vec<Tag>,
}

pub fn render_listing_page(
    page_title: &str,
    heading: &str,
    articles: &[Article],
    head_includes: &str,
    href_prefix: &str,
) -> String {
    // Group by year purely for labelling, assuming "YYYY-MM-DD".
    let mut body = String::new();

    let mut current_year: Option<i32> = None;

    for a in articles {
        let year = a.ctime.as_ref().map(|d| d.year());
        if year != current_year {
            if let Some(y) = year {
                body.push_str("<h2>");
                body.push_str(&escape_text(&y.to_string()));
                body.push_str("</h2>\n");
                current_year = year;
            } else {
                current_year = None;
            }
        }

        body.push_str(r#"<p class="meta">"#);
        if let Some(ctime) = &a.ctime {
            let ctime_str = ctime.as_str();
            body.push_str(r#" <time datetime=""#);
            body.push_str(&escape_attr(&ctime_str));
            body.push_str(r#"">"#);
            body.push_str(&escape_text(&ctime_str));
            body.push_str("</time>");
            body.push_str(r#"<span class="meta-sep">·</span>"#);
        }

        let full_href = format!("{href_prefix}{}", a.href.as_str());
        body.push_str(r#"<a href=""#);
        body.push_str(&escape_attr(&full_href));
        body.push_str(r#"">"#);
        body.push_str(&escape_text(&a.title));
        body.push_str("</a>");
        body.push_str("</p>\n");
    }

    crate::templates::listing_page(page_title, heading, &body, head_includes, href_prefix)
}

#[cfg(test)]
mod tests;
