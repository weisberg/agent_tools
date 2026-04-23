use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs::File,
    io::{Read, Seek},
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::error::DocliError;

#[derive(Clone, Debug)]
pub struct Package {
    pub path: PathBuf,
    pub source_hash: String,
    pub inventory: PartInventory,
    pub xml_parts: HashMap<String, Vec<u8>>,
    pub binary_parts: BTreeSet<String>,
}

#[derive(Clone, Debug, Default)]
pub struct PartInventory {
    pub entries: BTreeMap<String, PartEntry>,
}

#[derive(Clone, Debug)]
pub struct PartEntry {
    pub path: String,
    pub sha256: String,
    pub is_xml: bool,
    pub size_bytes: u64,
}

impl Package {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DocliError> {
        let path = path.as_ref().to_path_buf();
        if !path.exists() {
            return Err(DocliError::FileNotFound { path });
        }

        let source_hash = hash_file(&path)?;
        let reader = File::open(&path).map_err(|source| DocliError::CommitFailed {
            message: source.to_string(),
        })?;
        let mut archive = ZipArchive::new(reader)?;

        let mut inventory = PartInventory::default();
        let mut xml_parts = HashMap::new();
        let mut binary_parts = BTreeSet::new();

        for index in 0..archive.len() {
            let mut entry = archive.by_index(index)?;
            if entry.is_dir() {
                continue;
            }

            let name = entry.name().to_string();
            let mut bytes = Vec::with_capacity(entry.size() as usize);
            entry
                .read_to_end(&mut bytes)
                .map_err(|source| DocliError::InvalidDocx {
                    message: source.to_string(),
                })?;

            let is_xml = is_xml_part(&name);
            if is_xml {
                xml_parts.insert(name.clone(), bytes.clone());
            } else {
                binary_parts.insert(name.clone());
            }

            inventory.entries.insert(
                name.clone(),
                PartEntry {
                    path: name,
                    sha256: hash_bytes(&bytes),
                    is_xml,
                    size_bytes: bytes.len() as u64,
                },
            );
        }

        for required in ["[Content_Types].xml", "_rels/.rels", "word/document.xml"] {
            if !inventory.entries.contains_key(required) {
                return Err(DocliError::InvalidDocx {
                    message: format!("missing required DOCX part: {required}"),
                });
            }
        }

        Ok(Self {
            path,
            source_hash,
            inventory,
            xml_parts,
            binary_parts,
        })
    }

    pub fn entry_count(&self) -> usize {
        self.inventory.entries.len()
    }

    pub fn reopen_archive(&self) -> Result<ZipArchive<File>, DocliError> {
        let file = File::open(&self.path).map_err(|source| DocliError::CommitFailed {
            message: source.to_string(),
        })?;
        Ok(ZipArchive::new(file)?)
    }
}

fn hash_file(path: &Path) -> Result<String, DocliError> {
    let mut file = File::open(path).map_err(|source| DocliError::CommitFailed {
        message: source.to_string(),
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| DocliError::CommitFailed {
                message: source.to_string(),
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub(crate) fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn is_xml_part(path: &str) -> bool {
    path.ends_with(".xml") || path.ends_with(".rels")
}

pub(crate) fn copy_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<Vec<u8>, DocliError> {
    let mut entry = archive.by_name(name)?;
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .read_to_end(&mut bytes)
        .map_err(|source| DocliError::InvalidDocx {
            message: source.to_string(),
        })?;
    Ok(bytes)
}
