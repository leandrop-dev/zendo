//! GOG's changelog for a GOG game's page, as Heroic shows it.
//!
//! GOG gives it with the product, at
//! `api.gog.com/products/<id>?expand=changelog`, without a token: HTML,
//! or `null` for a game whose developer wrote none (about half the games
//! of a library). Its entries are split by `<hr>`, each opened by an
//! `<h4>` naming the update; later headings in an entry (`<h5>`, more
//! `<h4>`) name its parts, and the rest is paragraphs and lists. Zendo
//! reads it into [`Entry`]s rather than showing the HTML, keeping what it
//! says and dropping the styling.

use std::time::Duration;

use anyhow::{Context, bail};
use serde::Deserialize;

/// One update in the changelog.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    /// Its first heading, such as "Patch 2.5 Oct 22nd 2026".
    pub title: Option<String>,
    pub blocks: Vec<Block>,
}

/// A piece of an entry, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// A later heading, or a paragraph all in bold.
    Heading(String),
    /// A paragraph; its line breaks are kept as `\n`.
    Text(String),
    /// A list item; a line break in it is kept as `\n`.
    Item(String),
}

/// What a run of text being gathered will become.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text,
    Heading,
    Item,
}

/// A line break kept, apart from the other spaces closed up.
const BREAK: char = '\u{2028}';

/// GOG's changelog for the product `id` (the number in `gog:<id>`),
/// newest entry first; empty when GOG has none, or no longer knows the
/// product (it answers 404, "Product not found").
///
/// # Errors
///
/// When GOG cannot be reached, or answers with another error or with
/// something other than a product.
pub fn fetch(id: &str) -> anyhow::Result<Vec<Entry>> {
    #[derive(Deserialize)]
    struct Product {
        changelog: Option<String>,
    }
    let response = crate::http::answering_agent(Duration::from_secs(30))
        .get(format!("https://api.gog.com/products/{id}"))
        .query("expand", "changelog")
        .call()
        .context("could not reach GOG")?;
    match response.status().as_u16() {
        404 => return Ok(Vec::new()),
        200..=299 => {}
        status => bail!("GOG answered {status}"),
    }
    let text = response
        .into_body()
        .read_to_string()
        .context("GOG's answer broke off")?;
    let product: Product = serde_json::from_str(&text).context("GOG's answer is not a product")?;
    Ok(product
        .changelog
        .as_deref()
        .map(entries)
        .unwrap_or_default())
}

/// Reads GOG's changelog HTML into entries. Tags Zendo does not know
/// (links, emphasis, a stray `<key>` written as text) are dropped and
/// their text kept.
#[must_use]
pub fn entries(html: &str) -> Vec<Entry> {
    let mut reader = Reader::default();
    let mut rest = html;
    while let Some(open) = rest.find('<') {
        reader.text(&rest[..open]);
        let Some(close) = rest[open..].find('>') else {
            rest = &rest[open..];
            break;
        };
        reader.tag(&rest[open + 1..open + close]);
        rest = &rest[open + close + 1..];
    }
    reader.text(rest);
    reader.finish()
}

#[derive(Debug, Default)]
struct Reader {
    entries: Vec<Entry>,
    entry: Entry,
    gathered: String,
    kind: Option<Kind>,
    /// How many `<strong>` or `<b>` are open.
    bold: usize,
    /// Whether the run has words in bold, and words outside it.
    bold_words: bool,
    plain_words: bool,
}

impl Reader {
    fn text(&mut self, raw: &str) {
        if raw.is_empty() {
            return;
        }
        // A browser would join lines, but developers write one change a
        // line in a paragraph (Jupiter Hell's hundreds), so they are kept.
        let text = decode(raw)
            .replace("\r\n", "\n")
            .replace('\n', &BREAK.to_string());
        if text.chars().any(|c| !c.is_whitespace()) {
            if self.bold > 0 {
                self.bold_words = true;
            } else {
                self.plain_words = true;
            }
        }
        self.gathered.push_str(&text);
    }

    fn tag(&mut self, inside: &str) {
        let closing = inside.starts_with('/');
        let name: String = inside
            .trim_start_matches('/')
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect::<String>()
            .to_ascii_lowercase();
        match name.as_str() {
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.flush();
                self.kind = (!closing).then_some(Kind::Heading);
            }
            "li" => {
                self.flush();
                self.kind = (!closing).then_some(Kind::Item);
            }
            "ul" | "ol" => {
                self.flush();
                self.kind = None;
            }
            "p" | "div" | "br" if self.kind == Some(Kind::Item) => self.gathered.push(BREAK),
            "p" | "div" | "br" => self.flush(),
            "hr" => {
                self.flush();
                self.kind = None;
                let entry = std::mem::take(&mut self.entry);
                if entry.title.is_some() || !entry.blocks.is_empty() {
                    self.entries.push(entry);
                }
            }
            "strong" | "b" if closing => self.bold = self.bold.saturating_sub(1),
            "strong" | "b" => self.bold += 1,
            _ => {}
        }
    }

    /// Ends the run being gathered, as a block or the entry's title.
    fn flush(&mut self) {
        let gathered = std::mem::take(&mut self.gathered);
        let all_bold = self.bold_words && !self.plain_words;
        self.bold_words = false;
        self.plain_words = false;
        let text = tidy(&gathered);
        if text.is_empty() {
            return;
        }
        let kind = self.kind.unwrap_or(Kind::Text);
        let first = self.entry.title.is_none() && self.entry.blocks.is_empty();
        let block = match kind {
            Kind::Heading if first => {
                self.entry.title = Some(text.replace('\n', " "));
                return;
            }
            Kind::Heading => Block::Heading(text.replace('\n', " ")),
            Kind::Text if all_bold => Block::Heading(text.replace('\n', " ")),
            Kind::Text => Block::Text(text),
            Kind::Item => Block::Item(text),
        };
        self.entry.blocks.push(block);
    }

    fn finish(mut self) -> Vec<Entry> {
        self.tag("hr");
        self.entries
    }
}

/// Spaces closed up as a browser does, line breaks kept as `\n`, empty
/// lines dropped.
fn tidy(text: &str) -> String {
    text.split(BREAK)
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// HTML's character references written out; one Zendo does not know
/// stays as it is.
fn decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let found = rest
            .get(1..rest.len().min(12))
            .and_then(|after| after.find(';'))
            .and_then(|end| Some((reference(&rest[1..=end])?, end + 2)));
        if let Some((c, length)) = found {
            out.push(c);
            rest = &rest[length..];
        } else {
            out.push('&');
            rest = &rest[1..];
        }
    }
    out.push_str(rest);
    out
}

fn reference(name: &str) -> Option<char> {
    if let Some(number) = name.strip_prefix('#') {
        let code = match number.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => number.parse().ok()?,
        };
        return char::from_u32(code);
    }
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "rsquo" => '\u{2019}',
        "lsquo" => '\u{2018}',
        "ldquo" => '\u{201c}',
        "rdquo" => '\u{201d}',
        "nbsp" => ' ',
        "ndash" => '\u{2013}',
        "mdash" => '\u{2014}',
        "hellip" => '\u{2026}',
        "copy" => '\u{a9}',
        "reg" => '\u{ae}',
        "trade" => '\u{2122}',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(title: &str, blocks: Vec<Block>) -> Entry {
        Entry {
            title: Some(title.to_owned()),
            blocks,
        }
    }

    #[test]
    fn reads_entries_split_by_rules() {
        // As Cyberpunk 2077's: a bold paragraph naming a part, lists.
        let html = "<h4>Patch 2.5 Oct 22nd 2026</h4><p><strong>Edgerunners \
            Content</strong></p><ul><li>Added a new side quest, <i>Clampdown</i>.</li>\n\
            <li><strong>[TR1]</strong>  Fixed a softlock</li></ul><p> </p><hr>\
            <h4>Patch 2.31</h4><h5>Vehicles</h5><p>Patch notes:<br /> Fixed &quot;cars&quot; \
            &amp; bikes.</p>";
        assert_eq!(
            entries(html),
            [
                entry(
                    "Patch 2.5 Oct 22nd 2026",
                    vec![
                        Block::Heading("Edgerunners Content".into()),
                        Block::Item("Added a new side quest, Clampdown.".into()),
                        Block::Item("[TR1] Fixed a softlock".into()),
                    ]
                ),
                entry(
                    "Patch 2.31",
                    vec![
                        Block::Heading("Vehicles".into()),
                        Block::Text("Patch notes:".into()),
                        Block::Text("Fixed \"cars\" & bikes.".into()),
                    ]
                ),
            ]
        );
    }

    #[test]
    fn later_headings_in_an_entry_name_its_parts() {
        // As Frostpunk's and Bō's: several <h4> before the next rule,
        // empty ones, a list in a list, a paragraph in an item.
        let html = "<h4>Patch 1.5.0</h4><h4>The Last Autumn is out!</h4><h4></h4>\
            <ul><li>New<ul><li>Inner</li></ul></li><li><p>One</p><p>Two</p></li></ul>\
            <hr /><hr><h4>Update 1.05</h4><hr />";
        assert_eq!(
            entries(html),
            [
                entry(
                    "Patch 1.5.0",
                    vec![
                        Block::Heading("The Last Autumn is out!".into()),
                        Block::Item("New".into()),
                        Block::Item("Inner".into()),
                        Block::Item("One\nTwo".into()),
                    ]
                ),
                entry("Update 1.05", Vec::new()),
            ]
        );
    }

    #[test]
    fn writes_out_references_and_keeps_stray_text() {
        assert_eq!(
            decode("a&nbsp;b &#8217; &#x2014; &bogus; & AT&T"),
            "a b \u{2019} \u{2014} &bogus; & AT&T"
        );
        // Neverwinter Nights: Enhanced Edition writes `<key>` as text.
        assert_eq!(
            entries("<h4>Revision 28</h4><p>Set <key> to <name>.</p><p>1 < 2"),
            [entry(
                "Revision 28",
                vec![Block::Text("Set to .".into()), Block::Text("1 < 2".into())]
            )]
        );
        // Jupiter Hell's: a change a line in one paragraph.
        assert_eq!(
            entries("<h4>Patch 0.9.10</h4><p>NEW    #0683 - Medusa\r\nNEW #1353 -   big gun\n</p>"),
            [entry(
                "Patch 0.9.10",
                vec![Block::Text(
                    "NEW #0683 - Medusa\nNEW #1353 - big gun".into()
                )]
            )]
        );
        assert_eq!(entries(""), []);
        assert_eq!(
            entries("<p>Before any heading</p>"),
            [Entry {
                title: None,
                blocks: vec![Block::Text("Before any heading".into())]
            }]
        );
    }
}
