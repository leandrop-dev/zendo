//! GOG's changelog as a page draws it: one update's title and parts as lines,
//! the newest first, and what Show earlier updates says. Both windows use
//! these, so the words and the order stay one.

use zendo_core::gog_changelog::{Block, Entry};

use crate::t;

/// How many more entries Show earlier updates shows each time.
pub const MORE: usize = 3;

/// A line of the changelog card: 0 an update's title, 1 a heading, 2 a
/// paragraph, 3 a list item, 4 a gap between updates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub kind: i32,
    pub text: String,
}

/// The first `shown` updates as lines, each after the first opened by a gap.
#[must_use]
pub fn lines(entries: &[Entry], shown: usize) -> Vec<Line> {
    let shown = shown.min(entries.len());
    let mut lines = Vec::new();
    for (index, entry) in entries[..shown].iter().enumerate() {
        if index > 0 {
            lines.push(Line {
                kind: 4,
                text: String::new(),
            });
        }
        if let Some(title) = &entry.title {
            lines.push(Line {
                kind: 0,
                text: title.clone(),
            });
        }
        for block in &entry.blocks {
            lines.push(match block {
                Block::Heading(heading) => Line {
                    kind: 1,
                    text: heading.clone(),
                },
                Block::Text(paragraph) => Line {
                    kind: 2,
                    text: paragraph.clone(),
                },
                Block::Item(item) => Line {
                    kind: 3,
                    text: item.clone(),
                },
            });
        }
    }
    lines
}

/// What Show earlier updates says, when there are older updates.
#[must_use]
pub fn more_label(entries: &[Entry], shown: usize) -> Option<String> {
    let left = entries.len().saturating_sub(shown.min(entries.len()));
    (left > 0).then(|| t!("changelog-earlier", count = left))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_update_is_its_title_then_its_parts() {
        let entries = vec![
            Entry {
                title: Some("Patch 2".into()),
                blocks: vec![
                    Block::Heading("Fixed".into()),
                    Block::Text("Words".into()),
                    Block::Item("A thing".into()),
                ],
            },
            Entry {
                title: Some("Patch 1".into()),
                blocks: Vec::new(),
            },
        ];
        let one = lines(&entries, 1);
        let kinds: Vec<_> = one.iter().map(|line| line.kind).collect();
        assert_eq!(kinds, [0, 1, 2, 3]);
        let both = lines(&entries, 2);
        let kinds: Vec<_> = both.iter().map(|line| line.kind).collect();
        assert_eq!(kinds, [0, 1, 2, 3, 4, 0], "a gap opens the older update");
        assert!(more_label(&entries, 1).is_some());
        assert_eq!(more_label(&entries, 2), None);
    }
}
