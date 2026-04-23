use std::collections::HashMap;

use docli_core::Package;

/// Represents the data for a single part in the OOXML package.
#[derive(Clone, Debug)]
pub enum PartData {
    /// An XML part whose bytes have been loaded (possibly modified).
    Xml(Vec<u8>),
    /// A binary part whose bytes have been loaded.
    Binary(Vec<u8>),
    /// An unchanged part identified by path only — stream from source archive.
    Unchanged(String),
}

/// In-memory graph of all parts in a package, supporting mutation of XML parts.
#[derive(Clone, Debug)]
pub struct PartGraph {
    pub parts: HashMap<String, PartData>,
}

impl PartGraph {
    /// Build from a [`Package`] — XML parts as [`PartData::Xml`], binary parts as
    /// [`PartData::Unchanged`].
    pub fn from_package(package: &Package) -> Self {
        let mut parts = HashMap::new();

        for (path, bytes) in &package.xml_parts {
            parts.insert(path.clone(), PartData::Xml(bytes.clone()));
        }

        for path in &package.binary_parts {
            parts.insert(path.clone(), PartData::Unchanged(path.clone()));
        }

        Self { parts }
    }

    /// Get XML bytes for a part (`None` if binary/unchanged or missing).
    pub fn xml_bytes(&self, path: &str) -> Option<&[u8]> {
        match self.parts.get(path) {
            Some(PartData::Xml(bytes)) => Some(bytes),
            _ => None,
        }
    }

    /// Replace XML bytes for a part (marks it as touched).
    pub fn set_xml(&mut self, path: &str, bytes: Vec<u8>) {
        self.parts.insert(path.to_string(), PartData::Xml(bytes));
    }

    /// List all part paths.
    pub fn part_paths(&self) -> Vec<String> {
        self.parts.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_bytes_returns_xml_content() {
        let mut graph = PartGraph {
            parts: HashMap::new(),
        };
        graph
            .parts
            .insert("word/document.xml".into(), PartData::Xml(b"<doc/>".to_vec()));
        assert_eq!(graph.xml_bytes("word/document.xml"), Some(b"<doc/>".as_slice()));
    }

    #[test]
    fn xml_bytes_returns_none_for_binary() {
        let mut graph = PartGraph {
            parts: HashMap::new(),
        };
        graph
            .parts
            .insert("word/media/image1.png".into(), PartData::Binary(vec![0xFF]));
        assert!(graph.xml_bytes("word/media/image1.png").is_none());
    }

    #[test]
    fn set_xml_overwrites_part() {
        let mut graph = PartGraph {
            parts: HashMap::new(),
        };
        graph
            .parts
            .insert("word/document.xml".into(), PartData::Xml(b"<old/>".to_vec()));
        graph.set_xml("word/document.xml", b"<new/>".to_vec());
        assert_eq!(graph.xml_bytes("word/document.xml"), Some(b"<new/>".as_slice()));
    }

    #[test]
    fn part_paths_lists_all() {
        let mut graph = PartGraph {
            parts: HashMap::new(),
        };
        graph.parts.insert("a.xml".into(), PartData::Xml(vec![]));
        graph
            .parts
            .insert("b.png".into(), PartData::Unchanged("b.png".into()));
        let mut paths = graph.part_paths();
        paths.sort();
        assert_eq!(paths, vec!["a.xml", "b.png"]);
    }
}
