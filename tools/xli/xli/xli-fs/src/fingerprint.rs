use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use xli_core::XliError;

/// Compute a `sha256:` fingerprint for the given file.
pub fn fingerprint(path: &Path) -> Result<String, XliError> {
    let file = File::open(path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => XliError::FileNotFound {
            path: path.display().to_string(),
        },
        _ => XliError::OoxmlCorrupt {
            details: error.to_string(),
        },
    })?;

    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8 * 1024];

    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| XliError::OoxmlCorrupt {
                details: error.to_string(),
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("sha256:{:x}", hasher.finalize()))
}
