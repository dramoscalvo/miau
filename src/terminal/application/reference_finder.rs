//! Fuzzy search across saved reference documents.
//!
//! Every step contributes one entry for itself and one entry per nonblank artifact line, so a
//! query can jump to a step or straight to a point such as `BR-3` inside its document.

/// One searchable reference location: a step, or a line in that step's artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub node: usize,
    /// Zero-based line in the artifact; `None` selects the step without moving to a line.
    pub line: Option<usize>,
    pub step: String,
    pub text: String,
}

/// A chosen line the reference pane scrolls to once and then keeps highlighted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub line: usize,
    pub pending_scroll: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finder {
    pub query: String,
    pub selected: usize,
    entries: Vec<Entry>,
    matches: Vec<(usize, Vec<usize>)>,
}

impl Finder {
    pub fn new(entries: Vec<Entry>) -> Self {
        let mut finder = Self {
            query: String::new(),
            selected: 0,
            entries,
            matches: Vec::new(),
        };
        finder.refresh();
        finder
    }

    pub fn input(&mut self, character: char) {
        self.query.push(character);
        self.refresh();
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.refresh();
    }

    pub fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn select_next(&mut self) {
        self.selected = (self.selected + 1).min(self.matches.len().saturating_sub(1));
    }

    pub fn total(&self) -> usize {
        self.entries.len()
    }

    /// Ranked matches with the character positions of `text` that matched the query.
    pub fn matches(&self) -> impl Iterator<Item = (&Entry, &[usize])> {
        self.matches.iter().filter_map(|(index, positions)| {
            self.entries
                .get(*index)
                .map(|entry| (entry, positions.as_slice()))
        })
    }

    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    pub fn chosen(&self) -> Option<&Entry> {
        self.matches
            .get(self.selected)
            .and_then(|(index, _)| self.entries.get(*index))
    }

    fn refresh(&mut self) {
        let mut scored: Vec<_> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                fuzzy_match(&self.query, &entry.text)
                    .map(|(score, positions)| (score, index, positions))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        self.matches = scored
            .into_iter()
            .map(|(_, index, positions)| (index, positions))
            .collect();
        self.selected = 0;
    }
}

/// Build finder entries from each step's name, artifact filename, and saved artifact text.
pub fn entries<'a>(steps: impl IntoIterator<Item = (&'a str, &'a str, &'a str)>) -> Vec<Entry> {
    let mut entries = Vec::new();
    for (node, (name, writes, artifact)) in steps.into_iter().enumerate() {
        let step = format!("{}. {name}", node + 1);
        entries.push(Entry {
            node,
            line: None,
            step: step.clone(),
            text: format!("{step} · {writes}"),
        });
        entries.extend(
            artifact
                .lines()
                .enumerate()
                .filter(|(_, text)| !text.trim().is_empty())
                .map(|(line, text)| Entry {
                    node,
                    line: Some(line),
                    step: step.clone(),
                    text: text.trim().to_owned(),
                }),
        );
    }
    entries
}

/// Score `text` against whitespace-separated query terms; every term must match as a
/// case-insensitive subsequence. Consecutive characters and word starts score higher.
pub fn fuzzy_match(query: &str, text: &str) -> Option<(i64, Vec<usize>)> {
    let haystack: Vec<char> = text.chars().flat_map(char::to_lowercase).collect();
    if haystack.len() != text.chars().count() {
        // Lowercasing changed the length; fall back to exact characters to keep positions valid.
        return fuzzy_match_chars(query, &text.chars().collect::<Vec<_>>(), false);
    }
    fuzzy_match_chars(query, &haystack, true)
}

fn fuzzy_match_chars(query: &str, haystack: &[char], fold: bool) -> Option<(i64, Vec<usize>)> {
    let mut total = 0;
    let mut positions = Vec::new();
    for term in query.split_whitespace() {
        let needle: Vec<char> = if fold {
            term.chars().flat_map(char::to_lowercase).collect()
        } else {
            term.chars().collect()
        };
        let (score, matched) = best_term_match(&needle, haystack)?;
        total += score;
        positions.extend(matched);
    }
    positions.sort_unstable();
    positions.dedup();
    Some((total, positions))
}

fn best_term_match(needle: &[char], haystack: &[char]) -> Option<(i64, Vec<usize>)> {
    let first = *needle.first()?;
    haystack
        .iter()
        .enumerate()
        .filter(|(_, character)| **character == first)
        .filter_map(|(start, _)| greedy_match(needle, haystack, start))
        .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))
}

fn greedy_match(needle: &[char], haystack: &[char], start: usize) -> Option<(i64, Vec<usize>)> {
    let mut positions = Vec::with_capacity(needle.len());
    let mut from = start;
    for character in needle {
        let offset = haystack.get(from..)?.iter().position(|c| c == character)?;
        positions.push(from + offset);
        from += offset + 1;
    }
    let mut score = 0;
    let mut previous: Option<usize> = None;
    for &position in &positions {
        score += 16;
        let boundary = position == 0
            || haystack
                .get(position - 1)
                .is_some_and(|c| !c.is_alphanumeric());
        if boundary {
            score += 10;
        }
        if let Some(previous) = previous {
            let gap = position - previous - 1;
            score += if gap == 0 { 12 } else { -(gap.min(8) as i64) };
        }
        previous = Some(position);
    }
    Some((score, positions))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finder() -> Finder {
        Finder::new(entries([
            (
                "plan",
                "plan.md",
                "# Review\nD1: Which database?\n\nBR-3: Retain audit records.",
            ),
            (
                "review",
                "review.md",
                "Findings\nBR-3 is not covered by tests.",
            ),
        ]))
    }

    #[test]
    fn entries_cover_each_step_and_its_nonblank_lines() {
        let finder = finder();
        assert_eq!(finder.total(), 7);
        let lines: Vec<_> = finder
            .matches()
            .map(|(entry, _)| (entry.node, entry.line))
            .collect();
        assert_eq!(
            lines,
            [
                (0, None),
                (0, Some(0)),
                (0, Some(1)),
                (0, Some(3)),
                (1, None),
                (1, Some(0)),
                (1, Some(1)),
            ]
        );
    }

    #[test]
    fn query_terms_match_out_of_order_and_case_insensitively() {
        let mut finder = finder();
        for character in "audit br3".chars() {
            finder.input(character);
        }
        let chosen = finder.chosen().cloned();
        assert_eq!(
            chosen.map(|entry| (entry.node, entry.line)),
            Some((0, Some(3)))
        );
        assert_eq!(finder.match_count(), 1);
    }

    #[test]
    fn contiguous_word_start_matches_rank_above_scattered_ones() {
        let (tight, _) = fuzzy_match("br-3", "BR-3: Retain").unwrap_or_default();
        let (loose, _) = fuzzy_match("br-3", "abrupt-ending 3").unwrap_or_default();
        assert!(tight > loose, "{tight} <= {loose}");
    }

    #[test]
    fn positions_identify_matched_characters_for_highlighting() {
        assert_eq!(
            fuzzy_match("db", "D1: database").map(|(_, positions)| positions),
            Some(vec![4, 8])
        );
        assert_eq!(fuzzy_match("xyz", "D1: database"), None);
    }

    #[test]
    fn selection_stays_within_matches_and_resets_on_new_queries() {
        let mut finder = finder();
        for _ in 0..20 {
            finder.select_next();
        }
        assert_eq!(finder.selected, 6);
        finder.input('z');
        assert_eq!((finder.selected, finder.chosen()), (0, None));
        finder.select_next();
        finder.select_previous();
        assert_eq!(finder.selected, 0);
        finder.backspace();
        assert_eq!(finder.match_count(), 7);
    }
}
