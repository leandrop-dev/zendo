//! What the Collections sheet lists, the same in both windows: the
//! hand-picked collections and the saved views as one list by name, which
//! collections a set of games is in, and a saved view in a few words.

use std::collections::BTreeSet;

use zendo_core::{Category, Filter, SavedFilter};

use crate::format;

/// One collection of either kind, for the list.
pub enum Item<'a> {
    Picked(&'a Category),
    View(&'a SavedFilter),
}

impl Item<'_> {
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Picked(category) => &category.name,
            Self::View(saved) => &saved.name,
        }
    }
}

/// Every collection, hand-picked and saved views together, by name.
#[must_use]
pub fn items<'a>(categories: &'a [Category], filters: &'a [SavedFilter]) -> Vec<Item<'a>> {
    let mut items: Vec<Item<'a>> = categories
        .iter()
        .map(Item::Picked)
        .chain(filters.iter().map(Item::View))
        .collect();
    items.sort_by_cached_key(|item| (item.name().to_lowercase(), item.name().to_owned()));
    items
}

/// Whether every game the sheet was opened for is in the collection, which
/// is what its checkbox shows.
#[must_use]
pub fn member(targets: &BTreeSet<String>, category: &Category) -> bool {
    !targets.is_empty() && targets.iter().all(|id| category.games.contains(id))
}

/// A saved view in a few words: `FAVORITES · STEAM · BACKLOG · "doom"`.
#[must_use]
pub fn summary(filter: &Filter) -> String {
    let mut parts = vec![format::view(filter.view).to_uppercase()];
    if let Some(source) = filter.source {
        parts.push(format::source(source).to_uppercase());
    }
    if let Some(category) = &filter.category {
        parts.push(category.to_uppercase());
    }
    if let Some(status) = filter.status {
        parts.push(format::play_status(status).to_uppercase());
    }
    if !filter.search.trim().is_empty() {
        parts.push(format!("\u{201c}{}\u{201d}", filter.search.trim()));
    }
    parts.push(format::sort(filter.sort).to_uppercase());
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn category(name: &str, games: &[&str]) -> Category {
        Category {
            name: name.to_owned(),
            games: games.iter().map(|id| (*id).to_owned()).collect(),
        }
    }

    fn view(name: &str) -> SavedFilter {
        SavedFilter {
            name: name.to_owned(),
            filter: Filter::default(),
        }
    }

    fn targets(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(|id| (*id).to_owned()).collect()
    }

    #[test]
    fn ticks_only_when_every_target_is_in_the_collection() {
        let co_op = category("Co-op", &["a"]);
        assert!(
            !member(&targets(&[]), &co_op),
            "no targets, nothing to tick"
        );
        assert!(!member(&targets(&["a", "b"]), &co_op), "one of the two");
        assert!(member(&targets(&["a"]), &co_op));
    }

    #[test]
    fn lists_both_kinds_together_by_name() {
        let categories = [category("beta", &[]), category("Delta", &[])];
        let views = [view("Alpha"), view("charlie")];
        let names: Vec<String> = items(&categories, &views)
            .iter()
            .map(|item| item.name().to_owned())
            .collect();
        assert_eq!(names, ["Alpha", "beta", "charlie", "Delta"]);
    }
}
