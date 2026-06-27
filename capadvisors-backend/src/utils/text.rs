/// Splits `text` into overlapping chunks using a sliding window.
///
/// Each window is at most `max_words` wide. The next window starts at
/// `max_words - overlap_words` from the current start, so consecutive
/// chunks share `overlap_words` of trailing context — critical for
/// preventing sentence-boundary splits from losing meaning.
///
/// Panics in debug builds if `overlap_words >= max_words`.
pub fn sliding_window_chunks(text: &str, max_words: usize, overlap_words: usize) -> Vec<String> {
    debug_assert!(
        overlap_words < max_words,
        "overlap_words must be strictly smaller than max_words"
    );

    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }

    let step = max_words.saturating_sub(overlap_words);
    let mut chunks = Vec::new();
    let mut start = 0;

    while start < words.len() {
        let end = (start + max_words).min(words.len());
        chunks.push(words[start..end].join(" "));
        if end == words.len() {
            break;
        }
        start += step;
    }

    chunks
}

/// Counts whitespace-delimited words. Equivalent to `wc -w`.
pub fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_chunk_when_text_fits() {
        let text = "one two three four five";
        let chunks = sliding_window_chunks(text, 10, 2);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }

    #[test]
    fn overlap_carries_trailing_words() {
        // 6 words, window=4, overlap=2 → step=2
        // chunk 0: [0..4]  "a b c d"
        // chunk 1: [2..6]  "c d e f"
        let text = "a b c d e f";
        let chunks = sliding_window_chunks(text, 4, 2);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].ends_with("c d"));
        assert!(chunks[1].starts_with("c d"));
    }

    #[test]
    fn word_count_ignores_extra_whitespace() {
        assert_eq!(word_count("  hello   world  "), 2);
    }
}
