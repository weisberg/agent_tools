use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct CommitJournal {
    pub source_hash: String,
    pub output_hash: String,
    pub parts_modified: Vec<String>,
    pub parts_unchanged: usize,
    pub durability: String,
    pub revalidated: bool,
}
