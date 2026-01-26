/// Basic HTML shell shared by pages.
pub fn page_shell(
    head_common: &str,
    head_fragment: &str,
    body_header: &str,
    body: &str,
    footer: &str,
) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
{head_common}
{head_fragment}
</head>
<body>
<article>
<section>
{body_header}
{body}
</section>
</article>
</body>
{footer}
</html>
"#
    )
}

/// Render a listing page given shared head and href prefix.
pub fn listing_page(
    page_title: &str,
    heading: &str,
    body: &str,
    head_includes: &str,
    href_prefix: &str,
) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
{head_includes}
<title>{}</title>
<link rel="stylesheet" href="{}style.css">
</head>
<body>
<article>
<section>
<h1>{}</h1>
{}
</section>
</article>
</body>
</html>
"#,
        page_title, href_prefix, heading, body
    )
}
