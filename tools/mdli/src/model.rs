use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SectionInfo {
    pub(crate) id: Option<String>,
    pub(crate) path: String,
    pub(crate) title: String,
    pub(crate) level: usize,
    pub(crate) line: usize,
    pub(crate) start: usize,
    pub(crate) heading: usize,
    pub(crate) end: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct TableInfo {
    pub(crate) name: Option<String>,
    pub(crate) key: Option<String>,
    pub(crate) columns: Vec<String>,
    pub(crate) line: usize,
    pub(crate) marker: Option<usize>,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) section_id: Option<String>,
    pub(crate) section_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct BlockInfo {
    pub(crate) id: String,
    pub(crate) line: usize,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) checksum: Option<String>,
    pub(crate) locked: bool,
}

#[derive(Debug)]
pub(crate) struct DocumentIndex {
    pub(crate) sections: Vec<SectionInfo>,
    pub(crate) tables: Vec<TableInfo>,
    pub(crate) blocks: Vec<BlockInfo>,
    pub(crate) markers: Vec<MarkerAt>,
}

#[derive(Debug, Clone)]
pub(crate) struct MarkerAt {
    pub(crate) kind: String,
    pub(crate) fields: BTreeMap<String, String>,
    pub(crate) line: usize,
}
