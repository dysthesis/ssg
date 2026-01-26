//! Shared data types for the static site generator.
//! Implemented as newtypes to enforce invariants.

use std::{
    fmt,
    path::{Path, PathBuf},
};

use time::{Date, format_description};

/// Date format used for mtime and ctime.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IsoDate(Date);

impl IsoDate {
    pub fn parse(s: &str) -> Option<Self> {
        let fmt = format_description::parse("[year]-[month]-[day]").ok()?;
        Date::parse(s.trim(), &fmt).ok().map(Self)
    }

    pub fn as_str(&self) -> String {
        let fmt = format_description::parse("[year]-[month]-[day]")
            .expect("static date format string is valid");
        self.0.format(&fmt).unwrap_or_default()
    }

    pub fn year(&self) -> i32 {
        self.0.year()
    }

    pub fn as_date(&self) -> Date {
        self.0
    }
}

impl fmt::Display for IsoDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_str())
    }
}

/// Tags used to categorise articles.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tag(String);

impl Tag {
    pub fn parse(raw: &str) -> Option<Self> {
        if raw.is_empty() {
            return None;
        }
        let mut valid = true;
        for ch in raw.chars() {
            if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                continue;
            }
            valid = false;
            break;
        }
        if valid {
            Some(Self(raw.to_string()))
        } else {
            None
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The collection of tags for each article.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tags(pub Vec<Tag>);

impl Tags {
    pub fn new(tags: Vec<Tag>) -> Self {
        Self(tags)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<'a> IntoIterator for &'a Tags {
    type Item = &'a Tag;
    type IntoIter = std::slice::Iter<'a, Tag>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// Relative paths to internal content or assets.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RelPath(PathBuf);

impl RelPath {
    pub fn new(p: PathBuf) -> Option<Self> {
        if p.is_absolute() { None } else { Some(Self(p)) }
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Href(String);

impl Href {
    pub fn from_rel(rel: &RelPath) -> Self {
        let s = rel.as_path().to_string_lossy().replace('\\', "/");
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Href {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests;
