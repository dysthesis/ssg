pub const INPUT_DIR: &str = "contents";
pub const OUTPUT_DIR: &str = "public";
pub const POSTS_DIR: &str = "posts";
pub const TAGS_DIR: &str = "tags";

// Site-wide metadata used for feeds and absolute links.
pub const SITE_TITLE: &str = "Dysthesis";
pub const SITE_DESCRIPTION: &str = "Dysthesis' blog";
pub const SITE_BASE_URL: &str = "https://dysthesis.com/";
pub const SITE_AUTHOR: &str = "Dysthesis";
// Fallback image for OpenGraph/Twitter cards.
pub const SITE_DEFAULT_OG_IMAGE: Option<&str> = Some("assets/social-default.png");

/// Convenience container for site metadata used across rendering.
#[derive(Clone, Debug)]
pub struct SiteMeta {
    pub title: String,
    pub description: String,
    pub base_url: String,
    pub author: String,
    pub default_image: Option<String>,
}

pub fn site_meta() -> SiteMeta {
    SiteMeta {
        title: SITE_TITLE.to_string(),
        description: SITE_DESCRIPTION.to_string(),
        base_url: SITE_BASE_URL.trim_end_matches('/').to_string(),
        author: SITE_AUTHOR.to_string(),
        default_image: SITE_DEFAULT_OG_IMAGE.map(|s| s.to_string()),
    }
}

// Maximum number of items to include in feeds.
pub const FEED_ITEM_LIMIT: usize = 50;
