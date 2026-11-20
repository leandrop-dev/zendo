//! The window's words in the user's language, from Fluent files built into
//! the binary: `i18n/en.ftl`, and `i18n/pt-PT.ftl` for Portuguese
//! (Portugal). A message missing from a language comes in English; one
//! missing from English shows its id, which the tests rule out.
//!
//! One language serves the whole window: chosen at start from the
//! settings, or the system's, and changed from Settings with [`set`].
//! Words go through [`t!`]: `t!("settings-title")`, or with values,
//! `t!("covers-left", count = 3)`.

use std::sync::LazyLock;
use std::sync::atomic::{AtomicU8, Ordering};

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource};
use unic_langid::LanguageIdentifier;

/// A language the window speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    /// Portuguese as written in Portugal.
    Portuguese,
}

impl Language {
    pub const ALL: [Self; 2] = [Self::English, Self::Portuguese];

    /// Its tag, as `config.toml`'s `language` writes it.
    #[must_use]
    pub fn tag(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Portuguese => "pt-PT",
        }
    }

    /// Its name in its own words, as Settings lists it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Portuguese => "Português (Portugal)",
        }
    }

    /// The language a tag asks for: `en`, `en_GB.UTF-8`, `pt-PT`. Any
    /// Portuguese gets Portugal's, the only one there is [tentative].
    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        let language = tag
            .split(['-', '_', '.', '@'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        match language.as_str() {
            "en" => Some(Self::English),
            "pt" => Some(Self::Portuguese),
            _ => None,
        }
    }

    /// The system's: the first of its languages the window speaks, else
    /// English.
    #[must_use]
    pub fn system() -> Self {
        sys_locale::get_locales()
            .find_map(|tag| Self::from_tag(&tag))
            .unwrap_or(Self::English)
    }

    /// The one the settings choose (a tag), else the system's.
    pub fn chosen(tag: Option<&str>) -> Self {
        tag.and_then(Self::from_tag).unwrap_or_else(Self::system)
    }

    fn index(self) -> usize {
        self as usize
    }

    fn source(self) -> &'static str {
        match self {
            Self::English => include_str!("../i18n/en.ftl"),
            Self::Portuguese => include_str!("../i18n/pt-PT.ftl"),
        }
    }
}

static BUNDLES: LazyLock<[FluentBundle<FluentResource>; 2]> =
    LazyLock::new(|| Language::ALL.map(bundle));

static CURRENT: AtomicU8 = AtomicU8::new(0);

fn bundle(language: Language) -> FluentBundle<FluentResource> {
    let tag: LanguageIdentifier = language.tag().parse().unwrap_or_default();
    let mut bundle = FluentBundle::new_concurrent(vec![tag]);
    // The old Iced window drew the marks Fluent puts around values for
    // right-to-left text as boxes; neither language needs them.
    bundle.set_use_isolating(false);
    // A file with mistakes still gives the messages it could read; the
    // tests keep both free of them.
    let resource = FluentResource::try_new(language.source().to_owned())
        .unwrap_or_else(|(resource, _)| resource);
    bundle.add_resource_overriding(resource);
    bundle
}

/// Sets the language `config.toml` chooses, else the system's: for the
/// window, and for the dialogs Zendo runs as programs of their own. Reads
/// the file alone, leaving the keyring be.
pub fn set_from_settings() {
    let language = zendo_core::config::Config::load_file()
        .ok()
        .and_then(|config| config.language);
    set(Language::chosen(language.as_deref()));
}

/// The window's language.
pub fn current() -> Language {
    Language::ALL
        .get(usize::from(CURRENT.load(Ordering::Relaxed)))
        .copied()
        .unwrap_or(Language::English)
}

/// Changes the window's language; the next frame shows it.
pub fn set(language: Language) {
    let index = u8::try_from(language.index()).unwrap_or_default();
    CURRENT.store(index, Ordering::Relaxed);
}

/// A message in the window's language. Use [`t!`].
#[must_use]
pub fn tr(id: &str, args: Option<&FluentArgs>) -> String {
    format_in(current(), id, args)
        .or_else(|| format_in(Language::English, id, args))
        .unwrap_or_else(|| id.to_owned())
}

/// A message in a language, when that language has it.
#[must_use]
pub fn format_in(language: Language, id: &str, args: Option<&FluentArgs>) -> Option<String> {
    let bundle = &BUNDLES[language.index()];
    let pattern = bundle.get_message(id)?.value()?;
    let mut errors = Vec::new();
    Some(
        bundle
            .format_pattern(pattern, args, &mut errors)
            .into_owned(),
    )
}

/// Every message of a language that takes no values, formatted: the words a
/// frontend can show without an IPC call each. Messages with values go
/// through [`tr`] with their arguments instead.
#[must_use]
pub fn catalogue(language: Language) -> Vec<(String, String)> {
    let bundle = &BUNDLES[language.index()];
    let mut messages = Vec::new();
    for id in ids(language.source()) {
        let Some(pattern) = bundle.get_message(&id).and_then(|message| message.value()) else {
            continue;
        };
        let mut errors = Vec::new();
        let text = bundle.format_pattern(pattern, None, &mut errors);
        if errors.is_empty() {
            messages.push((id, text.into_owned()));
        }
    }
    messages.sort();
    messages
}

/// The window's language's messages without values: see [`catalogue`].
#[must_use]
pub fn current_catalogue() -> Vec<(String, String)> {
    catalogue(current())
}

/// The message ids an FTL file defines, read line by line: a message starts a
/// line with its id and ` =`; the lines under it are indented.
fn ids(source: &str) -> Vec<String> {
    let mut ids = Vec::new();
    for line in source.lines() {
        if let Some((id, _)) = line.split_once(" =")
            && !id.is_empty()
            && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            ids.push(id.to_owned());
        }
    }
    ids
}

/// A message in the window's language: `t!("id")`, or with values that
/// the message uses as `{ $name }`: `t!("id", name = value, count = 3)`.
#[macro_export]
macro_rules! t {
    ($id:literal) => {
        $crate::i18n::tr($id, None)
    };
    ($id:literal, $($name:ident = $value:expr),+ $(,)?) => {{
        let mut args = $crate::fluent_bundle::FluentArgs::new();
        $(args.set(stringify!($name), $crate::fluent_bundle::FluentValue::from($value));)+
        $crate::i18n::tr($id, Some(&args))
    }};
}
pub use crate::t;

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::Path;

    use super::*;

    /// Each message's id and the variables its text uses, read plainly:
    /// a message starts a line with `id =`; the lines under it, indented,
    /// belong to it.
    fn messages(source: &str) -> BTreeMap<String, BTreeSet<String>> {
        let mut messages: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut current = None;
        for line in source.lines() {
            if let Some((id, _)) = line.split_once(" =")
                && !id.is_empty()
                && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            {
                current = Some(id.to_owned());
                messages.entry(id.to_owned()).or_default();
            } else if !line.starts_with(' ') {
                current = None;
            }
            let Some(id) = &current else {
                continue;
            };
            let mut rest = line;
            while let Some((_, after)) = rest.split_once('$') {
                let name: String = after
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                    .collect();
                if let Some(variables) = messages.get_mut(id) {
                    variables.insert(name);
                }
                rest = after;
            }
        }
        messages
    }

    #[test]
    fn both_files_are_valid_fluent() {
        for language in Language::ALL {
            if let Err((_, errors)) = FluentResource::try_new(language.source().to_owned()) {
                panic!("{}: {errors:?}", language.tag());
            }
            let mut bundle = FluentBundle::new_concurrent(Vec::new());
            let resource = FluentResource::try_new(language.source().to_owned()).unwrap();
            if let Err(errors) = bundle.add_resource(resource) {
                panic!("{}: a message twice: {errors:?}", language.tag());
            }
        }
    }

    #[test]
    fn every_language_indexes_its_own_bundle() {
        for language in Language::ALL {
            assert_eq!(Language::ALL[language.index()], language);
        }
    }

    #[test]
    fn both_languages_have_the_same_messages_with_the_same_values() {
        let english = messages(Language::English.source());
        let portuguese = messages(Language::Portuguese.source());
        let missing: Vec<&String> = english
            .keys()
            .filter(|id| !portuguese.contains_key(*id))
            .collect();
        let extra: Vec<&String> = portuguese
            .keys()
            .filter(|id| !english.contains_key(*id))
            .collect();
        assert!(missing.is_empty(), "not in pt-PT.ftl: {missing:?}");
        assert!(extra.is_empty(), "only in pt-PT.ftl: {extra:?}");
        for (id, variables) in &english {
            assert_eq!(
                portuguese.get(id),
                Some(variables),
                "{id} uses other values in pt-PT.ftl"
            );
        }
    }

    /// Every `t!("…")` in the window's code names a message English has,
    /// and every message is used.
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one walk over every kind of source the words appear in"
    )]
    fn every_message_used_exists() {
        let english = messages(Language::English.source());
        // The `src` of every crate in the workspace: the window's code is
        // in more than one.
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        // Each crate's `ui` folder too, where there is one: the window's
        // frontend asks `t("id")` and `tr("id", …)` from its own `ui`.
        let mut files: Vec<_> = fs::read_dir(&crates)
            .unwrap()
            .flat_map(|entry| {
                let path = entry.unwrap().path();
                [path.join("src"), path.join("ui")]
            })
            .filter(|dir| dir.is_dir())
            .collect();
        let mut unknown = Vec::new();
        let mut used = BTreeSet::new();
        while let Some(path) = files.pop() {
            if path.is_dir() {
                // Builds and installed packages carry copies of the sources.
                if matches!(
                    path.file_name().and_then(|name| name.to_str()),
                    Some("node_modules" | ".svelte-kit" | "build" | "target" | "gen" | "pkg")
                ) {
                    continue;
                }
                files.extend(
                    fs::read_dir(&path)
                        .unwrap()
                        .map(|entry| entry.unwrap().path()),
                );
                continue;
            }
            let text_of = |path: &Path| fs::read_to_string(path).unwrap();
            if path
                .extension()
                .is_some_and(|ext| ext == "svelte" || ext == "ts" || ext == "js")
            {
                let text = text_of(&path);
                // `t("id")` and `tr("id", values)`: the second takes values.
                for call in ["t(", "tr("] {
                    let mut rest = text.as_str();
                    while let Some((before, after)) = rest.split_once(call) {
                        rest = after;
                        // `t(`, not the end of `format(` or `list(`.
                        if before
                            .chars()
                            .next_back()
                            .is_some_and(|c| c.is_alphanumeric() || c == '_')
                        {
                            continue;
                        }
                        let Some(quote @ ('"' | '\'')) = after.chars().next() else {
                            continue;
                        };
                        let id = after[1..].split(quote).next().unwrap_or_default();
                        if id.is_empty() {
                            continue;
                        }
                        if !english.contains_key(id) {
                            unknown.push(format!("{}: {id}", path.display()));
                        }
                        used.insert(id.to_owned());
                    }
                }
                continue;
            }
            if path.extension().is_none_or(|ext| ext != "rs") || path.ends_with("i18n.rs") {
                continue;
            }
            let text = text_of(&path);
            let mut rest = text.as_str();
            while let Some((before, after)) = rest.split_once("t!(") {
                rest = after;
                // `t!`, not the end of `format!` or `print!`; rustfmt may
                // put the id on a line of its own.
                let Some(after) = after.trim_start().strip_prefix('"') else {
                    continue;
                };
                if before
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_')
                {
                    continue;
                }
                let id = after.split('"').next().unwrap_or_default();
                used.insert(id.to_owned());
                if !english.contains_key(id) {
                    unknown.push(format!("{}: {id}", path.display()));
                }
            }
            // `i18n::tr("id", values)`: a message with values, from the word's
            // own function rather than the macro.
            let mut rest = text.as_str();
            while let Some((before, after)) = rest.split_once("tr(\"") {
                rest = after;
                if before
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_')
                {
                    continue;
                }
                let id = after.split('"').next().unwrap_or_default();
                used.insert(id.to_owned());
                if !english.contains_key(id) {
                    unknown.push(format!("{}: {id}", path.display()));
                }
            }
        }
        assert!(unknown.is_empty(), "not in en.ftl: {unknown:?}");
        let unused: Vec<&String> = english.keys().filter(|id| !used.contains(*id)).collect();
        assert!(unused.is_empty(), "never used: {unused:?}");
    }

    /// Each message, given each value it uses, formats without an error in
    /// both languages, counted as 1 and as 2.
    #[test]
    fn every_message_formats_cleanly() {
        for language in Language::ALL {
            let bundle = &BUNDLES[language.index()];
            for (id, variables) in messages(language.source()) {
                for count in [1, 2] {
                    let mut args = FluentArgs::new();
                    for name in &variables {
                        args.set(name.clone(), count);
                    }
                    let pattern = bundle.get_message(&id).and_then(|m| m.value()).unwrap();
                    let mut errors = Vec::new();
                    bundle.format_pattern(pattern, Some(&args), &mut errors);
                    assert!(errors.is_empty(), "{}: {id}: {errors:?}", language.tag());
                }
            }
        }
    }

    #[test]
    fn reads_the_systems_tags() {
        assert_eq!(Language::from_tag("en_GB.UTF-8"), Some(Language::English));
        assert_eq!(Language::from_tag("pt-PT"), Some(Language::Portuguese));
        assert_eq!(Language::from_tag("pt_BR"), Some(Language::Portuguese));
        assert_eq!(Language::from_tag("C"), None);
        assert_eq!(Language::from_tag("de-DE"), None);
        assert_eq!(Language::chosen(Some("pt-PT")), Language::Portuguese);
    }

    /// The catalogue has every message that takes no values, and none that
    /// does: those go through `t!` with their arguments.
    #[test]
    fn the_catalogue_is_the_messages_that_need_no_values() {
        let catalogue = catalogue(Language::English);
        assert!(
            catalogue
                .iter()
                .any(|(id, text)| id == "page-library" && text == "Library")
        );
        let listed: BTreeSet<String> = catalogue.into_iter().map(|(id, _)| id).collect();
        for (id, variables) in messages(Language::English.source()) {
            assert_eq!(listed.contains(&id), variables.is_empty(), "{id}");
        }
    }

    #[test]
    fn counts_in_each_language() {
        let mut args = FluentArgs::new();
        args.set("count", 1);
        assert_eq!(
            format_in(Language::English, "count-games", Some(&args)).as_deref(),
            Some("1 game")
        );
        assert_eq!(
            format_in(Language::Portuguese, "count-games", Some(&args)).as_deref(),
            Some("1 jogo")
        );
        args.set("count", 2);
        assert_eq!(
            format_in(Language::Portuguese, "count-games", Some(&args)).as_deref(),
            Some("2 jogos")
        );
        // Portugal's Portuguese says 0 in the plural, unlike Brazil's.
        args.set("count", 0);
        assert_eq!(
            format_in(Language::Portuguese, "count-games", Some(&args)).as_deref(),
            Some("0 jogos")
        );
    }
}
