use std::collections::HashSet;

use docli_core::Package;

/// Tracks all numeric IDs found in a package's XML parts and allocates new unique IDs.
pub struct IdAllocator {
    used_ids: HashSet<u64>,
    next_id: u64,
}

impl IdAllocator {
    /// Scan all XML parts in a package for `w:id`, `w:commentId`, `r:id` attribute values.
    /// Build a set of all used IDs.
    pub fn from_package(package: &Package) -> Self {
        let mut used_ids = HashSet::new();

        for bytes in package.xml_parts.values() {
            let xml_str = match std::str::from_utf8(bytes) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let doc = match roxmltree::Document::parse(xml_str) {
                Ok(d) => d,
                Err(_) => continue,
            };

            for node in doc.descendants() {
                if !node.is_element() {
                    continue;
                }
                for attr in node.attributes() {
                    let local = attr.name();
                    // Match id, Id, commentId and similar ID-bearing attributes.
                    if local == "id" || local == "Id" || local == "commentId" {
                        // Try parsing the value as a bare u64 first.
                        if let Ok(v) = attr.value().parse::<u64>() {
                            used_ids.insert(v);
                        } else {
                            // Extract trailing numeric part (e.g. "rId3" → 3).
                            let numeric: String =
                                attr.value().chars().rev().take_while(|c| c.is_ascii_digit()).collect();
                            if !numeric.is_empty() {
                                let numeric: String = numeric.chars().rev().collect();
                                if let Ok(v) = numeric.parse::<u64>() {
                                    used_ids.insert(v);
                                }
                            }
                        }
                    }
                }
            }
        }

        let next_id = used_ids.iter().copied().max().map_or(1, |m| m + 1);

        Self { used_ids, next_id }
    }

    /// Allocate the next unused ID. Panics if `>= 0x7FFF_FFFF`.
    pub fn next(&mut self) -> u64 {
        assert!(self.next_id < 0x7FFF_FFFF, "ID space exhausted");
        while self.used_ids.contains(&self.next_id) {
            self.next_id += 1;
            assert!(self.next_id < 0x7FFF_FFFF, "ID space exhausted");
        }
        let id = self.next_id;
        self.used_ids.insert(id);
        self.next_id += 1;
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_allocator(ids: &[u64]) -> IdAllocator {
        let used_ids: HashSet<u64> = ids.iter().copied().collect();
        let next_id = used_ids.iter().copied().max().map_or(1, |m| m + 1);
        IdAllocator { used_ids, next_id }
    }

    #[test]
    fn next_returns_sequential_ids() {
        let mut alloc = make_allocator(&[]);
        assert_eq!(alloc.next(), 1);
        assert_eq!(alloc.next(), 2);
        assert_eq!(alloc.next(), 3);
    }

    #[test]
    fn next_skips_used_ids() {
        let mut alloc = make_allocator(&[1, 2, 3, 5]);
        // next_id starts at 6
        assert_eq!(alloc.next(), 6);
        assert_eq!(alloc.next(), 7);
    }

    #[test]
    fn next_fills_gap_if_forced() {
        let mut alloc = IdAllocator {
            used_ids: HashSet::from([1, 3]),
            next_id: 2,
        };
        assert_eq!(alloc.next(), 2);
        assert_eq!(alloc.next(), 4);
    }
}
