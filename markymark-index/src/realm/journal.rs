//! Journal date lookup methods for the realm index.

use markymark_core::DocumentUri;

use super::RealmIndex;

impl RealmIndex {
    /// Returns all journal documents for a given year and month, sorted by day ascending.
    /// The tuple is `(DocumentUri, day)` so callers can sort or filter by specific dates.
    pub fn lookup_journal_by_month(&self, year: u16, month: u8) -> Vec<(DocumentUri, u8)> {
        let start = (year, month, 1u8);
        let end = (year, month, 31u8);
        self.date_to_docs
            .range(start..=end)
            .flat_map(|((_, _, d), uris)| uris.iter().map(move |u| (u.clone(), *d)))
            .collect()
    }

    /// Returns the detected journal date `(year, month, day)` for a URI, or `None`
    /// if the URI does not correspond to a journal page.
    pub fn journal_date(&self, uri: &DocumentUri) -> Option<(u16, u8, u8)> {
        self.uri_to_date.get(uri.as_str()).copied()
    }
}
