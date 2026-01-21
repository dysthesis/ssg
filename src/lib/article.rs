use crate::transformer::toc::{escape_attr, escape_text};

#[derive(Clone, Debug)]
pub struct Article {
    pub title: String,
    // ISO-8601 date, "YYYY-MM-DD".
    pub ctime: String,
    pub href: String,
    pub tags: Vec<String>,
}

pub fn render_listing_page(
    page_title: &str,
    heading: &str,
    articles: &[Article],
    head_includes: &str,
    href_prefix: &str,
) -> String {
    // Group by year purely for labelling. Assumes "YYYY-MM-DD".
    let mut out = String::new();

    out.push_str(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
"#,
    );
    out.push_str(head_includes);
    out.push('\n');
    out.push_str("<title>");
    out.push_str(&escape_text(page_title));
    out.push_str("</title>\n");
    out.push_str(&format!(
        r#"<link rel="stylesheet" href="{}style.css">"#,
        escape_attr(href_prefix)
    ));
    out.push_str(
        r#"
</head>
<body>
<article>
<section>
"#,
    );

    out.push_str("<h1>");
    out.push_str(&escape_text(heading));
    out.push_str("</h1>\n");

    let mut current_year: Option<&str> = None;

    for a in articles {
        let year = a.ctime.get(0..4);
        if year != current_year {
            if let Some(y) = year {
                out.push_str("<h2>");
                out.push_str(&escape_text(y));
                out.push_str("</h2>\n");
                current_year = year;
            } else {
                current_year = None;
            }
        }

        out.push_str(r#"<p class="meta">"#);
        if !a.ctime.is_empty() {
            out.push_str(r#" <time datetime=""#);
            out.push_str(&escape_attr(&a.ctime));
            out.push_str(r#"">"#);
            out.push_str(&escape_text(&a.ctime));
            out.push_str("</time>");
            out.push_str(r#"<span class="meta-sep">·</span>"#);
        }

        let full_href = format!("{href_prefix}{}", a.href);
        out.push_str(r#"<a href=""#);
        out.push_str(&escape_attr(&full_href));
        out.push_str(r#"">"#);
        out.push_str(&escape_text(&a.title));
        out.push_str("</a>");
        out.push_str("</p>\n");
    }

    out.push_str(
        r#"
</section>
</article>
</body>
</html>
"#,
    );

    out
}
