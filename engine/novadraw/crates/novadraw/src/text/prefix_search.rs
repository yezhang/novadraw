use unicode_segmentation::UnicodeSegmentation;

/// Reuses the candidate buffer and the best measured result. As in the previous
/// truncation search, the binary probe order is preserved even when shaping
/// contextual prefixes does not give monotone widths.
pub(crate) fn fit_prefix<L, E>(
    source: &str,
    suffix: &str,
    empty: Option<L>,
    mut measure: impl FnMut(&str) -> Result<L, E>,
    fits: impl Fn(&L) -> bool,
) -> Result<(usize, Option<L>), E> {
    let offsets = source
        .grapheme_indices(true)
        .map(|(offset, _)| offset)
        .chain(std::iter::once(source.len()))
        .collect::<Vec<_>>();
    let count = offsets.len() - 1;
    let mut low = 0;
    let mut high = count;
    let mut best = empty;
    let mut candidate = String::new();
    while low < high {
        let probe = (low + high).div_ceil(2);
        candidate.clear();
        candidate.push_str(&source[..offsets[probe]]);
        candidate.push_str(suffix);
        let measured = measure(&candidate)?;
        if fits(&measured) {
            low = probe;
            best = Some(measured);
        } else {
            high = probe - 1;
        }
    }
    Ok((offsets[low], best))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_preserves_grapheme_boundaries_and_does_not_remeasure_the_winner() {
        let source = "e\u{301}👩‍💻中abc";
        let prefixes = source
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .chain(std::iter::once(source.len()))
            .collect::<Vec<_>>();
        for limit in 0..prefixes.len() {
            let mut visited = Vec::new();
            let (end, winner) = fit_prefix(
                source,
                "...",
                Some(String::from("...")),
                |candidate| {
                    visited.push(candidate.to_owned());
                    Ok::<_, ()>(candidate.to_owned())
                },
                |candidate| candidate.trim_end_matches('.').graphemes(true).count() <= limit,
            )
            .unwrap();
            assert_eq!(end, prefixes[limit.min(prefixes.len() - 1)]);
            assert_eq!(winner.unwrap(), format!("{}...", &source[..end]));
            let unique = visited.iter().collect::<std::collections::HashSet<_>>();
            assert_eq!(
                unique.len(),
                visited.len(),
                "no candidate is measured twice"
            );
        }
    }

    #[test]
    fn empty_input_and_measurement_errors_keep_the_original_contract() {
        let result = fit_prefix("", "...", Some(7), |_| Err::<i32, _>("called"), |_| true);
        assert_eq!(result, Ok((0, Some(7))));
        let result = fit_prefix(
            "abc",
            "...",
            None,
            |_| Err::<i32, _>("font failed"),
            |_| true,
        );
        assert_eq!(result, Err("font failed"));
    }
}
