use std::{fs, path::Path};

use chrono::{DateTime, FixedOffset, Utc};
use color_eyre::eyre::eyre;
use rss::{Category, Channel, Guid, Item};

use crate::{
    article::Article,
    config::{FEED_ITEM_LIMIT, SITE_AUTHOR, SITE_BASE_URL, SITE_DESCRIPTION, SITE_TITLE},
    types::{IsoDate, Tag},
};

/// Minimal site metadata used for feed generation.
#[derive(Debug)]
pub struct SiteMeta {
    pub title: String,
    pub description: String,
    pub base_url: String,
    pub author: String,
}

/// Generate both RSS and Atom feeds into the given output directory.
pub fn write_feeds(out_dir: &Path, articles: &[Article]) -> color_eyre::Result<()> {
    let meta = SiteMeta {
        title: SITE_TITLE.to_string(),
        description: SITE_DESCRIPTION.to_string(),
        base_url: SITE_BASE_URL.to_string(),
        author: SITE_AUTHOR.to_string(),
    };

    let entries = articles
        .iter()
        .take(FEED_ITEM_LIMIT)
        .map(|a| FeedEntry::from_article(a, &meta.base_url))
        .collect::<Vec<_>>();

    let rss_xml = build_rss(&entries, &meta)?;
    fs::write(out_dir.join("rss.xml"), rss_xml)?;

    let atom_xml = build_atom(&entries, &meta)?;
    fs::write(out_dir.join("atom.xml"), atom_xml)?;

    Ok(())
}

#[derive(Clone, Debug)]
struct FeedEntry {
    title: String,
    url: String,
    summary: Option<String>,
    tags: Vec<Tag>,
    published: Option<IsoDate>,
    updated: Option<IsoDate>,
}

impl FeedEntry {
    fn from_article(article: &Article, base_url: &str) -> Self {
        let base = base_url.trim_end_matches('/');
        let url = format!("{}/{}", base, article.href.as_str());

        Self {
            title: article.title.clone(),
            url,
            summary: article.summary.clone(),
            tags: article.tags.clone(),
            published: article.ctime.clone(),
            updated: article.updated.clone().or_else(|| article.ctime.clone()),
        }
    }
}

fn build_rss(entries: &[FeedEntry], meta: &SiteMeta) -> color_eyre::Result<String> {
    let mut channel = Channel::default();
    channel.set_title(meta.title.clone());
    channel.set_link(meta.base_url.clone());
    channel.set_description(meta.description.clone());

    let mut items = Vec::with_capacity(entries.len());
    for entry in entries {
        let mut item = Item::default();
        item.set_title(Some(entry.title.clone()));
        item.set_link(Some(entry.url.clone()));
        item.set_guid(Some(Guid {
            value: entry.url.clone(),
            permalink: true,
        }));

        if let Some(date) = entry.updated.as_ref().or(entry.published.as_ref()) {
            item.set_pub_date(Some(to_rfc2822(date)?));
        }

        if let Some(summary) = &entry.summary {
            item.set_description(Some(summary.clone()));
        }

        if !entry.tags.is_empty() {
            let cats: Vec<Category> = entry
                .tags
                .iter()
                .map(|t| {
                    let mut c = Category::default();
                    c.set_name(t.as_str().to_string());
                    c
                })
                .collect();
            item.set_categories(cats);
        }

        items.push(item);
    }

    channel.set_items(items);
    Ok(channel.to_string())
}

fn build_atom(entries: &[FeedEntry], meta: &SiteMeta) -> color_eyre::Result<String> {
    let mut feed = atom_syndication::Feed::default();
    feed.set_title(meta.title.clone());
    feed.set_id(meta.base_url.clone());

    // Updated is required in Atom; use newest entry or fallback to now.
    let updated = entries
        .first()
        .and_then(|e| e.updated.as_ref())
        .map(to_chrono)
        .transpose()?
        .unwrap_or_else(|| DateTime::<FixedOffset>::from(Utc::now()));
    feed.set_updated(updated);

    {
        let mut link = atom_syndication::Link::default();
        link.set_href(meta.base_url.clone());
        feed.set_links(vec![link]);
    }

    {
        let mut author = atom_syndication::Person::default();
        author.set_name(meta.author.clone());
        feed.set_authors(vec![author]);
    }

    let mut atom_entries = Vec::with_capacity(entries.len());
    for entry in entries {
        let mut e = atom_syndication::Entry::default();
        e.set_id(entry.url.clone());
        e.set_title(entry.title.clone());
        let entry_updated = entry
            .updated
            .as_ref()
            .map(to_chrono)
            .transpose()?
            .unwrap_or_else(|| updated);
        e.set_updated(entry_updated);

        let mut link = atom_syndication::Link::default();
        link.set_href(entry.url.clone());
        e.set_links(vec![link]);

        if let Some(summary) = &entry.summary {
            let mut content = atom_syndication::Content::default();
            content.set_content_type(Some("html".into()));
            content.set_value(Some(summary.clone()));
            e.set_content(Some(content));
        }

        if !entry.tags.is_empty() {
            let categories: Vec<atom_syndication::Category> = entry
                .tags
                .iter()
                .map(|t| {
                    let mut c = atom_syndication::Category::default();
                    c.set_term(t.as_str().to_string());
                    c
                })
                .collect();
            e.set_categories(categories);
        }

        atom_entries.push(e);
    }

    feed.set_entries(atom_entries);
    Ok(feed.to_string())
}

fn to_chrono(date: &IsoDate) -> color_eyre::Result<DateTime<FixedOffset>> {
    let s = format!("{}T00:00:00+00:00", date.as_str());
    DateTime::parse_from_rfc3339(&s).map_err(|e| eyre!("parse date: {e}"))
}

fn to_rfc2822(date: &IsoDate) -> color_eyre::Result<String> {
    Ok(to_chrono(date)?.to_rfc2822())
}
