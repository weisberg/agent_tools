use crate::index::HeadingEntry;

pub fn resolve_heading_path(headings: &[HeadingEntry], path: &str) -> Option<usize> {
    let wanted = path
        .split('/')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if wanted.is_empty() {
        return None;
    }

    let mut stack = Vec::new();
    for heading in headings {
        let level = heading.level.max(1) as usize;
        if stack.len() >= level {
            stack.truncate(level - 1);
        }
        stack.push(heading.text.trim());
        if stack == wanted {
            return Some(heading.paragraph_index);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use crate::index::HeadingEntry;

    use super::resolve_heading_path;

    #[test]
    fn resolves_nested_heading_path() {
        let headings = vec![
            HeadingEntry {
                paragraph_index: 0,
                level: 1,
                text: "Results".to_string(),
            },
            HeadingEntry {
                paragraph_index: 3,
                level: 2,
                text: "CUPED".to_string(),
            },
        ];

        assert_eq!(resolve_heading_path(&headings, "Results/CUPED"), Some(3));
    }
}
