use similar::{ChangeTag, TextDiff};

/// Compare deux corps HTML. Retourne None si identiques, sinon un diff unifie tronque.
pub fn compute_diff(a: &str, b: &str, max_lines: usize) -> Option<String> {
    if a == b {
        return None;
    }
    let diff = TextDiff::from_lines(a, b);
    let mut out = String::new();
    let mut count = 0;
    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            ChangeTag::Delete => '-',
            ChangeTag::Insert => '+',
            ChangeTag::Equal => ' ',
        };
        // Skip context lines to keep output compact
        if matches!(change.tag(), ChangeTag::Equal) {
            continue;
        }
        out.push(sign);
        out.push_str(change.value());
        if !change.value().ends_with('\n') {
            out.push('\n');
        }
        count += 1;
        if count >= max_lines {
            out.push_str(&format!("... (diff tronque a {} lignes)\n", max_lines));
            break;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_identical_returns_none() {
        assert!(compute_diff("a\nb\n", "a\nb\n", 100).is_none());
    }

    #[test]
    fn test_diff_different_returns_unified() {
        let d = compute_diff("a\nb\n", "a\nc\n", 100).unwrap();
        assert!(d.contains("-b"));
        assert!(d.contains("+c"));
    }

    #[test]
    fn test_diff_truncation() {
        let a = (0..500).map(|i| format!("line{}\n", i)).collect::<String>();
        let b = (500..1000).map(|i| format!("line{}\n", i)).collect::<String>();
        let d = compute_diff(&a, &b, 10).unwrap();
        assert!(d.contains("diff tronque"));
    }
}
