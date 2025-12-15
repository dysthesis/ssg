//! Utilities for loading and validating benchmark corpus files

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const CORPUS_DIR: &str = "benches/corpora";
const MANIFEST_PATH: &str = "benches/corpora/manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusFileMetadata {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub family: String,
    pub event_count: usize,
    pub code_blocks: usize,
    pub inline_math: usize,
    pub display_math: usize,
    pub tables: usize,
    pub footnotes: usize,
    pub metadata_blocks: usize,
    pub strikethrough: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub description: String,
    pub files: Vec<CorpusFileMetadata>,
}

impl Manifest {
    /// Load the manifest from disk
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(MANIFEST_PATH)?;
        let manifest: Manifest = serde_json::from_str(&content)?;
        Ok(manifest)
    }

    /// Get metadata for a specific file by path
    pub fn get(&self, path: &str) -> Option<&CorpusFileMetadata> {
        self.files.iter().find(|f| f.path == path)
    }

    /// Get all files in a specific family
    pub fn family(&self, family: &str) -> Vec<&CorpusFileMetadata> {
        self.files.iter().filter(|f| f.family == family).collect()
    }
}

/// A loaded corpus file with validated contents
#[derive(Debug, Clone)]
pub struct CorpusFile {
    pub metadata: CorpusFileMetadata,
    pub content: String,
    pub path: PathBuf,
}

impl CorpusFile {
    /// Load a corpus file and validate its hash
    pub fn load(relative_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let manifest = Manifest::load()?;
        let metadata = manifest
            .get(relative_path)
            .ok_or_else(|| format!("File not found in manifest: {}", relative_path))?;

        let full_path = PathBuf::from(CORPUS_DIR).join(relative_path);
        let content = fs::read_to_string(&full_path)?;

        // Verify hash
        let hash = Self::compute_sha256(&content);
        if hash != metadata.sha256 {
            return Err(format!(
                "Hash mismatch for {}: expected {}, got {}",
                relative_path, metadata.sha256, hash
            )
            .into());
        }

        // Verify size
        if content.len() != metadata.size_bytes as usize {
            return Err(format!(
                "Size mismatch for {}: expected {}, got {}",
                relative_path,
                metadata.size_bytes,
                content.len()
            )
            .into());
        }

        Ok(Self {
            metadata: metadata.clone(),
            content,
            path: full_path,
        })
    }

    /// Compute SHA-256 hash of content
    fn compute_sha256(content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Get the size in bytes
    pub fn size_bytes(&self) -> usize {
        self.metadata.size_bytes as usize
    }

    /// Get the expected event count
    pub fn event_count(&self) -> usize {
        self.metadata.event_count
    }

    /// Get the content as a string slice
    pub fn as_str(&self) -> &str {
        &self.content
    }
}

/// Preload multiple corpus files
pub struct CorpusCache {
    files: HashMap<String, CorpusFile>,
}

impl CorpusCache {
    /// Create a new empty cache
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    /// Load a file into the cache
    pub fn load(&mut self, relative_path: &str) -> Result<&CorpusFile, Box<dyn std::error::Error>> {
        if !self.files.contains_key(relative_path) {
            let file = CorpusFile::load(relative_path)?;
            self.files.insert(relative_path.to_string(), file);
        }
        Ok(&self.files[relative_path])
    }

    /// Load all files from a family
    pub fn load_family(
        &mut self,
        family: &str,
    ) -> Result<Vec<&CorpusFile>, Box<dyn std::error::Error>> {
        let manifest = Manifest::load()?;
        let family_files = manifest.family(family);

        for metadata in family_files {
            self.load(&metadata.path)?;
        }

        Ok(self
            .files
            .values()
            .filter(|f| f.metadata.family == family)
            .collect())
    }

    /// Get a cached file
    pub fn get(&self, relative_path: &str) -> Option<&CorpusFile> {
        self.files.get(relative_path)
    }
}

impl Default for CorpusCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to load a corpus file for benchmarking with panic on error
pub fn load_corpus(relative_path: &str) -> CorpusFile {
    CorpusFile::load(relative_path)
        .unwrap_or_else(|e| panic!("Failed to load corpus file {}: {}", relative_path, e))
}

/// Load a snippet file (doesn't need hash validation as they're small)
pub fn load_snippet(name: &str) -> String {
    let path = PathBuf::from(CORPUS_DIR).join("snippets").join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("Failed to load snippet {}: {}", name, e))
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn test_manifest_loads() {
        let manifest = Manifest::load().expect("Failed to load manifest");
        assert!(!manifest.files.is_empty());
        assert_eq!(manifest.version, "1.0");
    }

    #[test]
    fn test_corpus_file_loads_and_validates() {
        let file = CorpusFile::load("plain/1k.md").expect("Failed to load corpus file");
        assert!(!file.content.is_empty());
        assert_eq!(file.metadata.family, "plain");
    }

    #[test]
    fn test_corpus_cache() {
        let mut cache = CorpusCache::new();
        cache.load("plain/1k.md").expect("Failed to load");

        let file = cache.get("plain/1k.md").expect("File should be cached");
        assert!(!file.content.is_empty());
    }

    #[test]
    fn test_load_family() {
        let mut cache = CorpusCache::new();
        let files = cache.load_family("plain").expect("Failed to load family");
        assert!(!files.is_empty());
    }
}
