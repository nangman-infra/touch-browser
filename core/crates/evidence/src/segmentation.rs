pub(crate) fn segment_block_text(text: &str) -> Vec<String> {
    split_sentence_boundaries(text)
        .into_iter()
        .flat_map(|sentence| split_clause_boundaries(&sentence))
        .filter(|segment| !segment.trim().is_empty())
        .collect()
}

fn split_sentence_boundaries(text: &str) -> Vec<String> {
    let chars = text.char_indices().collect::<Vec<_>>();
    let mut segments = Vec::new();
    let mut start = 0usize;

    for (index, character) in &chars {
        let is_boundary = match *character {
            '.' => is_period_boundary(text, *index),
            '!' | '?' | ';' | '\n' | '\r' => true,
            _ => false,
        };
        if !is_boundary {
            continue;
        }

        let segment = text[start..*index].trim();
        if !segment.is_empty() {
            segments.push(segment.to_string());
        }
        start = index + character.len_utf8();
    }

    let trailing = text[start..].trim();
    if !trailing.is_empty() {
        segments.push(trailing.to_string());
    }

    segments
}

fn is_period_boundary(text: &str, index: usize) -> bool {
    let previous = text[..index].chars().next_back();
    let immediate_next = text[index + 1..].chars().next();
    let next = text[index + 1..]
        .chars()
        .find(|character| !character.is_whitespace());

    if immediate_next.is_some_and(char::is_whitespace) {
        return true;
    }

    !matches!(
        (previous, next),
        (Some(left), Some(right)) if left.is_alphanumeric() && right.is_alphanumeric()
    )
}

fn split_clause_boundaries(text: &str) -> Vec<String> {
    let mut segments = vec![text.trim().to_string()];

    for marker in [
        ", but ",
        " but ",
        ", however ",
        " however ",
        ", while ",
        " while ",
        " whereas ",
    ] {
        segments = split_segments_on_marker(segments, marker);
    }

    segments
}

fn split_segments_on_marker(segments: Vec<String>, marker: &str) -> Vec<String> {
    segments
        .into_iter()
        .flat_map(|segment| split_segment_on_marker(&segment, marker))
        .collect()
}

fn split_segment_on_marker(segment: &str, marker: &str) -> Vec<String> {
    let Some(split_index) = segment.to_ascii_lowercase().find(marker) else {
        return vec![segment.trim().to_string()];
    };

    let left = segment[..split_index].trim();
    let right = segment[split_index + marker.len()..].trim();
    let mut parts = Vec::new();

    if !left.is_empty() {
        parts.push(left.to_string());
    }
    if !right.is_empty() {
        parts.push(right.to_string());
    }

    if parts.is_empty() {
        vec![segment.trim().to_string()]
    } else {
        parts
    }
}

#[cfg(test)]
mod tests {
    use super::segment_block_text;

    #[test]
    fn preserves_simple_sentences_as_single_segments() {
        assert_eq!(
            segment_block_text("Python is a programming language."),
            vec!["Python is a programming language".to_string()]
        );
    }

    #[test]
    fn does_not_split_domain_or_runtime_names_on_periods() {
        assert_eq!(
            segment_block_text("Node.js is an asynchronous runtime. example.com is reserved."),
            vec![
                "Node.js is an asynchronous runtime".to_string(),
                "example.com is reserved".to_string(),
            ]
        );
    }

    #[test]
    fn splits_but_clauses_for_mixed_qualifier_sentences() {
        let segments = segment_block_text(
            "The default value is 3 seconds, but the maximum value is 900 seconds (15 minutes).",
        );

        assert_eq!(
            segments,
            vec![
                "The default value is 3 seconds".to_string(),
                "the maximum value is 900 seconds (15 minutes)".to_string(),
            ]
        );
    }
}
