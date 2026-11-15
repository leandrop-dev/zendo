//! Nexus Mods in the window, without a toolkit: the key on the Settings page
//! and what opens `nxm://` links, a link that arrives (which game it is for,
//! the download into that game's mods), the newer files found for a game's
//! mods and the Update on a mod. The Iced window and the Slint one each draw
//! it; the work that waits on the network runs on a thread through the
//! functions at the end ([`match_games`], [`fetch`], [`ask_updates`],
//! [`check_key`], [`become_handler`]).
//!
//! [`NexusState`] holds what the window remembers. Its methods are the
//! transitions: each returns what the window is to do next (a job for a
//! thread, a line for the footer), and none touches a file or the network,
//! so tests need no user files. `runnable` is false when the window cannot
//! do the work (a test): the state changes the same, but nothing is started.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use zendo_core::Game;
use zendo_core::mods::{self, Added, NexusSource, Overview};
use zendo_core::nexus::{self, FileInfo, Files, Handler, KeyCheck, Link, User};

use crate::t;

/// The newer files of a game's mods, and the mods whose file list could not
/// be asked.
pub type UpdateList = (Vec<(String, FileInfo)>, Vec<String>);

/// What Nexus said of a link's game: the installed games it matched (id and
/// title), every installed game, and Nexus's name for the game.
#[derive(Debug, Clone)]
pub struct Matched {
    pub link: Link,
    pub matched: Vec<(String, String)>,
    pub installed: Vec<(String, String)>,
    pub name: String,
}

/// What the page says once the key is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Then {
    Removed,
    /// Saved after Nexus accepted it, and whose key it is.
    Saved(String, User),
    /// Saved although Nexus could not be reached.
    Unchecked(String),
}

/// The key's field and what it last said.
#[derive(Debug, Default)]
pub struct KeyField {
    pub value: String,
    pub shown: bool,
    /// The key Nexus is being asked about, so an answer for another is let
    /// go.
    pub checking: Option<String>,
    pub save_after_check: bool,
    /// What happened, and whether it went well.
    pub note: Option<(String, bool)>,
}

/// What a press on Save or Check leaves to do.
#[derive(Debug, PartialEq, Eq)]
pub enum KeyStep {
    /// Only the state changed.
    Nothing,
    /// The field was emptied: the key is removed.
    Remove,
    /// Nexus is to be asked about this key, on a thread.
    Check(String),
}

/// What Nexus's answer about the key leaves to do.
#[derive(Debug, PartialEq, Eq)]
pub enum Checked {
    Nothing,
    /// Write the key, and say `Then`.
    Save(String, Then),
}

/// A link no single game matched: the user says which game.
#[derive(Debug, Clone)]
pub struct Choosing {
    pub link: Link,
    /// Nexus's name for the game.
    pub name: String,
    /// The games to choose from: id and title.
    pub games: Vec<(String, String)>,
    /// No game matched, so every installed one is offered.
    pub all: bool,
}

/// What a link that arrived leaves to do.
#[derive(Debug, PartialEq, Eq)]
pub enum Arrived {
    /// A line for the footer, and nothing more.
    Say(String),
    /// The library is not read yet: the link waits.
    Wait,
    /// Which game it is for is to be asked, on a thread (the line says the
    /// mod arrived).
    Ask(Link, String),
}

/// What the games a link matched leave to do.
#[derive(Debug, PartialEq, Eq)]
pub enum Matching {
    /// One game: download into it.
    Download(String, Link),
    Say(String),
    /// The chooser is open.
    Choose,
}

/// A download that was begun: what its thread needs.
#[derive(Debug)]
pub struct Started {
    pub id: u64,
    pub stop: Arc<AtomicBool>,
    pub note: String,
}

/// What a finished download leaves to do.
#[derive(Debug)]
pub enum Finished {
    /// An archive with an installer: its options are to be chosen.
    Choose(Box<mods::Pending>),
    /// The mod is in: the game's mods as they stand, and a line.
    Done(Box<Overview>, String),
    Say(String),
}

/// What Update on a mod leaves to do.
#[derive(Debug, PartialEq, Eq)]
pub enum Update {
    /// Premium: the file is downloaded straight away.
    Download(Link),
    /// Otherwise the file's page opens, for the site's button to send it
    /// back; the line says so.
    Page(String, String),
}

/// A game whose mods' updates are to be asked, on a thread.
#[derive(Debug)]
pub struct UpdateJob {
    pub game: String,
    pub sources: Vec<(String, NexusSource)>,
}

/// One mod downloading from Nexus.
#[derive(Debug)]
pub struct Download {
    pub id: u64,
    pub game: String,
    /// The file on Nexus.
    pub file: u64,
    pub name: String,
    pub done: u64,
    pub total: Option<u64>,
    stop: Arc<AtomicBool>,
}

impl Download {
    /// Whether Stop was pressed and the download has not ended yet.
    #[must_use]
    pub fn stopping(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }

    /// How far it got, as a fraction for a bar (nothing when the size is not
    /// known) and in words.
    #[must_use]
    pub fn progress(&self) -> (f32, String) {
        let mib = |bytes: u64| {
            #[expect(
                clippy::cast_precision_loss,
                reason = "shown to one decimal; the precision lost is far below"
            )]
            let mib = bytes as f32 / (1024.0 * 1024.0);
            mib
        };
        match self.total {
            Some(total) if total > 0 => (
                (mib(self.done) / mib(total).max(f32::MIN_POSITIVE)).clamp(0.0, 1.0),
                t!(
                    "nexus-progress",
                    done = format!("{:.1}", mib(self.done)),
                    total = format!("{:.1}", mib(total))
                ),
            ),
            _ => (
                0.0,
                t!(
                    "nexus-progress-unknown",
                    done = format!("{:.1}", mib(self.done))
                ),
            ),
        }
    }
}

/// What the window remembers of Nexus.
#[derive(Debug, Default)]
pub struct NexusState {
    pub key: KeyField,
    /// Whose key it is, once Nexus said.
    pub user: Option<User>,
    /// What opens `nxm://` links, once asked.
    pub handler: Option<Handler>,
    /// Links that came before the library was read.
    pub pending: Vec<String>,
    pub choosing: Option<Choosing>,
    pub downloads: Vec<Download>,
    next: u64,
    /// Newer files for a game's mods, by game and mod.
    updates: HashMap<String, HashMap<String, FileInfo>>,
    /// Games whose mods were checked this session.
    checked: HashSet<String>,
}

impl NexusState {
    /// The settings were read: the field shows the stored key.
    pub fn loaded(&mut self, stored: Option<&str>) {
        stored.unwrap_or_default().clone_into(&mut self.key.value);
    }

    /// Whether the key's account has Premium; `None` before Nexus said.
    #[must_use]
    pub fn premium(&self) -> Option<bool> {
        self.user.as_ref().map(|user| user.is_premium)
    }

    /// Says on the Settings page what became of the key once it was
    /// written, and whose key Zendo now has.
    pub fn key_stored(&mut self, then: Then) {
        match then {
            Then::Removed => {
                self.user = None;
                self.key.note = Some((t!("key-removed"), true));
            }
            Then::Saved(who, user) => {
                self.user = Some(user);
                // Updates are asked again with the new key.
                self.checked.clear();
                self.key.note = Some((t!("nexus-key-saved", who = who), true));
            }
            Then::Unchecked(error) => {
                self.key.note = Some((t!("key-saved-unchecked", error = error), false));
            }
        }
    }

    pub fn edit_key(&mut self, value: String) {
        self.key.value = value;
        self.key.note = None;
    }

    pub fn toggle_shown(&mut self) {
        self.key.shown = !self.key.shown;
    }

    /// Save: an empty field removes the key; any other is asked about first
    /// and written once Nexus has answered.
    pub fn save_key(&mut self, runnable: bool) -> KeyStep {
        let key = self.key.value.trim().to_owned();
        self.key.value.clone_from(&key);
        if key.is_empty() {
            return KeyStep::Remove;
        }
        self.key.save_after_check = true;
        self.begin_check(key, runnable)
    }

    /// Check: asks Nexus about the key in the field, and writes nothing.
    pub fn check_key(&mut self, runnable: bool) -> KeyStep {
        self.key.save_after_check = false;
        let key = self.key.value.trim().to_owned();
        self.begin_check(key, runnable)
    }

    fn begin_check(&mut self, key: String, runnable: bool) -> KeyStep {
        if key.is_empty() {
            self.key.note = Some((t!("key-paste-first"), false));
            return KeyStep::Nothing;
        }
        self.key.checking = Some(key.clone());
        self.key.note = None;
        if runnable {
            KeyStep::Check(key)
        } else {
            KeyStep::Nothing
        }
    }

    /// Nexus answered about `key`; `stored` is the key the settings hold.
    pub fn key_checked(
        &mut self,
        key: &str,
        result: Result<KeyCheck, String>,
        stored: Option<&str>,
    ) -> Checked {
        match self.key.checking.as_deref() {
            // The check at start, with nothing typed: it is the stored key.
            None => {
                if let Ok(KeyCheck::Accepted(user)) = result {
                    self.user = Some(user);
                }
                return Checked::Nothing;
            }
            Some(checking) if checking == key => {}
            // An answer about a key the field has moved on from.
            Some(_) => return Checked::Nothing,
        }
        self.key.checking = None;
        let save = std::mem::take(&mut self.key.save_after_check);
        // The check proves the key in the field. That is Zendo's key only
        // when it is the one stored, and only then does its account tell
        // whether Premium applies.
        let is_stored = stored == Some(key);
        match result {
            Ok(KeyCheck::Accepted(user)) => {
                let who = signed_in(&user);
                if save {
                    return Checked::Save(key.to_owned(), Then::Saved(who, user));
                }
                if is_stored {
                    self.user = Some(user);
                    // Updates are asked again with the new key.
                    self.checked.clear();
                }
                self.key.note = Some((t!("nexus-key-accepted", who = who), true));
            }
            Ok(KeyCheck::Rejected) => {
                let note = if save {
                    t!("nexus-key-rejected-unsaved")
                } else {
                    t!("nexus-key-rejected")
                };
                self.key.note = Some((note, false));
            }
            Err(err) if save => return Checked::Save(key.to_owned(), Then::Unchecked(err)),
            Err(err) => self.key.note = Some((err, false)),
        }
        Checked::Nothing
    }

    /// What opens links, as `xdg-mime` said.
    pub fn handler_known(&mut self, handler: Handler) {
        self.handler = Some(handler);
    }

    /// The handler row's line, whether Zendo is the handler, and whether
    /// the button that makes it so is pressable.
    #[must_use]
    pub fn handler_row(&self) -> (String, bool, bool) {
        let (line, is_zendo) = match &self.handler {
            Some(Handler::Zendo) => (t!("nexus-handler-zendo"), true),
            Some(Handler::Other(other)) => {
                (t!("nexus-handler-other", other = other.as_str()), false)
            }
            Some(Handler::OtherZendo(program)) => (
                t!("nexus-handler-other-zendo", program = program.as_str()),
                false,
            ),
            Some(Handler::Nothing) => (t!("nexus-handler-nothing"), false),
            None => (String::new(), false),
        };
        (line, is_zendo, !is_zendo && self.handler.is_some())
    }

    /// The handler was set: the line to say.
    pub fn became_handler(&mut self, result: Result<String, String>) -> String {
        match result {
            Ok(exe) => {
                self.handler = Some(Handler::Zendo);
                t!("nexus-links-open-in", exe = exe)
            }
            Err(err) => err,
        }
    }

    /// A link from the browser. `library` is whether the library was read
    /// and `key` whether Zendo has a Nexus key.
    pub fn link_arrived(&mut self, text: &str, library: bool, key: bool) -> Arrived {
        let link = match Link::parse(text) {
            Ok(link) => link,
            Err(err) => return Arrived::Say(format!("{err:#}")),
        };
        if !library {
            self.pending.push(text.to_owned());
            return Arrived::Wait;
        }
        if !key {
            return Arrived::Say(t!("nexus-needs-key"));
        }
        Arrived::Ask(link, t!("nexus-mod-arrived"))
    }

    /// The games the link matched.
    pub fn matched(&mut self, found: Matched) -> Matching {
        let Matched {
            link,
            matched,
            installed,
            name,
        } = found;
        if let [(id, _)] = matched.as_slice() {
            return Matching::Download(id.clone(), link);
        }
        if installed.is_empty() {
            return Matching::Say(t!("nexus-no-game", name = name.as_str()));
        }
        let all = matched.is_empty();
        self.choosing = Some(Choosing {
            link,
            name,
            games: if all { installed } else { matched },
            all,
        });
        Matching::Choose
    }

    /// A game was chosen: the game, the link and whether Nexus's game is
    /// kept as that game's.
    pub fn choose(&mut self, id: &str) -> Option<(String, Link, bool)> {
        let choosing = self.choosing.take()?;
        choosing
            .games
            .iter()
            .any(|(game, _)| game == id)
            .then(|| (id.to_owned(), choosing.link, choosing.all))
    }

    pub fn cancel_choose(&mut self) {
        self.choosing = None;
    }

    /// Begins a download of the link's file into `game`'s mods; nothing when
    /// that file is on its way already.
    pub fn begin(&mut self, game: &Game, link: &Link) -> Option<Started> {
        if self
            .downloads
            .iter()
            .any(|download| download.game == game.id && download.file == link.file_id)
        {
            return None;
        }
        self.next += 1;
        let id = self.next;
        let stop = Arc::new(AtomicBool::new(false));
        self.downloads.push(Download {
            id,
            game: game.id.clone(),
            file: link.file_id,
            name: t!(
                "nexus-download-name",
                title = game.title.as_str(),
                file = link.file_id
            ),
            done: 0,
            total: None,
            stop: Arc::clone(&stop),
        });
        Some(Started {
            id,
            stop,
            note: t!("nexus-downloading", title = game.title.as_str()),
        })
    }

    pub fn progress(&mut self, id: u64, done: u64, total: Option<u64>) {
        if let Some(download) = self.downloads.iter_mut().find(|d| d.id == id) {
            download.done = done;
            download.total = total;
        }
    }

    pub fn stop(&mut self, id: u64) {
        if let Some(download) = self.downloads.iter().find(|d| d.id == id) {
            download.stop.store(true, Ordering::Relaxed);
        }
    }

    /// The downloads into one game's mods.
    pub fn downloads_for<'a>(&'a self, game: &'a str) -> impl Iterator<Item = &'a Download> + 'a {
        self.downloads
            .iter()
            .filter(move |download| download.game == game)
    }

    /// A download ended.
    pub fn finished(
        &mut self,
        id: u64,
        game: &str,
        result: Result<(Added, String), String>,
    ) -> Finished {
        let stopped = self
            .downloads
            .iter()
            .find(|download| download.id == id)
            .is_some_and(Download::stopping);
        self.downloads.retain(|download| download.id != id);
        match result {
            Ok((Added::Choose(pending), _)) => Finished::Choose(pending),
            Ok((Added::Done(overview), note)) => {
                // An update found before is done once its file is in.
                if let Some(updates) = self.updates.get_mut(game) {
                    updates.retain(|mod_id, file| {
                        !overview.mods.iter().any(|info| {
                            &info.id == mod_id
                                && info
                                    .nexus
                                    .as_ref()
                                    .is_some_and(|nexus| nexus.file_id == file.file_id)
                        })
                    });
                }
                Finished::Done(overview, note)
            }
            Err(_) if stopped => Finished::Say(t!("nexus-download-stopped")),
            Err(err) => Finished::Say(t!("nexus-download-failed", error = err)),
        }
    }

    /// The game's mods are read: whether Nexus is to be asked for their
    /// newer files (once a session, and only for mods that came from Nexus).
    pub fn want_updates(
        &mut self,
        game: &str,
        overview: &Overview,
        key: bool,
        runnable: bool,
    ) -> Option<UpdateJob> {
        if !key {
            return None;
        }
        let sources: Vec<(String, NexusSource)> = overview
            .mods
            .iter()
            .filter_map(|info| info.nexus.clone().map(|nexus| (info.id.clone(), nexus)))
            .collect();
        if sources.is_empty() || !runnable || !self.checked.insert(game.to_owned()) {
            return None;
        }
        Some(UpdateJob {
            game: game.to_owned(),
            sources,
        })
    }

    /// Nexus listed the newer files; the line to say when some list failed.
    pub fn updates_found(
        &mut self,
        game: &str,
        result: Result<UpdateList, String>,
    ) -> Option<String> {
        match result {
            Ok((found, problems)) => {
                self.updates
                    .insert(game.to_owned(), found.into_iter().collect());
                // One mod's file list failing does not hide the others'
                // updates, but it is still said.
                (!problems.is_empty())
                    .then(|| t!("nexus-updates-failed", error = problems.join("; ")))
            }
            Err(err) => Some(t!("nexus-updates-failed", error = err)),
        }
    }

    /// The newer file Nexus has for a mod, when one was found.
    #[must_use]
    pub fn update_for(&self, game: &str, mod_id: &str) -> Option<&FileInfo> {
        self.updates.get(game)?.get(mod_id)
    }

    /// Update on a mod of `game`, whose mods are `overview`.
    #[must_use]
    pub fn update(&self, game: &str, mod_id: &str, overview: &Overview) -> Option<Update> {
        let file = self.update_for(game, mod_id)?;
        let source = overview
            .mods
            .iter()
            .find(|info| info.id == mod_id)?
            .nexus
            .clone()?;
        let link = Link {
            domain: source.domain,
            mod_id: source.mod_id,
            file_id: file.file_id,
            key: None,
            expires: None,
        };
        if self.premium() == Some(true) {
            return Some(Update::Download(link));
        }
        Some(Update::Page(link.page(), t!("nexus-press-mod-manager")))
    }
}

/// The words of a key's account.
#[must_use]
pub fn signed_in(user: &User) -> String {
    if user.is_premium {
        t!("nexus-known-as-premium", name = user.name.as_str())
    } else {
        t!("nexus-known-as", name = user.name.as_str())
    }
}

/// The label of Update on a mod, for the newer file.
#[must_use]
pub fn update_label(file: &FileInfo) -> String {
    match file
        .version
        .as_deref()
        .filter(|version| !version.is_empty())
    {
        Some(version) => t!("mods-update-to", version = version),
        None => t!("mods-update"),
    }
}

/// The games with a folder to put mods in: the ones a link can be for.
#[must_use]
pub fn with_folders(games: &[Game]) -> Vec<Game> {
    games
        .iter()
        .filter(|game| {
            game.details
                .install_path
                .as_ref()
                .is_some_and(|dir| dir.is_dir())
        })
        .cloned()
        .collect()
}

// The work that waits on the network or the disk, for a thread.

/// Asks which games a link is for: Nexus's game list (at most weekly) names
/// its game, and the installed games whose Nexus domain it is match.
#[must_use]
pub fn match_games(key: &str, link: &Link, games: &[Game]) -> Matched {
    let _ = nexus::refresh_games(key);
    let name = nexus::game_name(&link.domain, &nexus::cached_games());
    let pair = |game: &Game| (game.id.clone(), game.title.clone());
    let matched = games
        .iter()
        .filter(|game| mods::nexus_domain(game).as_deref() == Some(link.domain.as_str()))
        .map(pair)
        .collect();
    Matched {
        link: link.clone(),
        matched,
        installed: games.iter().map(pair).collect(),
        name,
    }
}

/// Downloads a file from Nexus and adds it to the game's mods, saying how far
/// it got; `remember` keeps the game as the link's game on Nexus.
///
/// # Errors
///
/// The words of what went wrong.
pub fn fetch(
    game: &Game,
    key: &str,
    link: &Link,
    remember: bool,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<(Added, String), String> {
    (|| {
        if remember {
            mods::set_nexus(game, &link.domain)?;
        }
        let folder = mods::downloads_dir(game)?;
        let fetched = nexus::fetch(key, link, &folder, stop, progress)?;
        let (added, updated) = mods::add_from_nexus(game, &fetched, &link.domain, link.mod_id)?;
        anyhow::Ok((
            added,
            crate::mods::added_note(updated, &fetched, &game.title),
        ))
    })()
    .map_err(|err| format!("{err:#}"))
}

/// Asks Nexus for the newer files of mods that came from it.
#[must_use]
pub fn ask_updates(key: &str, sources: Vec<(String, NexusSource)>) -> UpdateList {
    let mut found = Vec::new();
    let mut problems = Vec::new();
    let mut asked: HashMap<(String, u64), Files> = HashMap::new();
    for (mod_id, source) in sources {
        let wanted = (source.domain.clone(), source.mod_id);
        let files = match asked.entry(wanted) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                match nexus::files(key, &source.domain, source.mod_id) {
                    Ok(files) => entry.insert(files),
                    Err(err) => {
                        // One mod's file list failing is not the whole
                        // game's.
                        problems.push(format!("{}: {err:#}", source.domain));
                        continue;
                    }
                }
            }
        };
        if let Some(newer) = files.newest(source.file_id) {
            found.push((mod_id, newer.clone()));
        }
    }
    (found, problems)
}

/// Asks Nexus about a key. At start (`refresh`), Nexus's list of games is
/// brought up to date on the way, at most weekly.
///
/// # Errors
///
/// The words of what went wrong.
pub fn check_key(key: &str, refresh: bool) -> Result<KeyCheck, String> {
    if refresh {
        let _ = nexus::refresh_games(key);
    }
    nexus::check_key(key).map_err(|err| format!("{err:#}"))
}

/// Makes this Zendo open `nxm://` links; the program's path on success.
///
/// # Errors
///
/// The words of what went wrong.
pub fn become_handler() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|err| t!("nexus-no-exe", error = err.to_string()))?;
    nexus::become_handler(&exe)
        .map(|_| exe.display().to_string())
        .map_err(|err| format!("{err:#}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(premium: bool) -> User {
        User {
            name: "someone".into(),
            is_premium: premium,
        }
    }

    fn file(id: u64) -> FileInfo {
        FileInfo {
            file_id: id,
            name: "New".into(),
            version: Some("2".into()),
            file_name: "a.zip".into(),
            size_kb: None,
            category_name: None,
            uploaded_timestamp: 0,
        }
    }

    #[test]
    fn a_saved_key_is_checked_first_and_written_only_when_accepted() {
        let mut state = NexusState::default();
        state.edit_key("good".into());
        assert_eq!(state.save_key(true), KeyStep::Check("good".into()));
        assert_eq!(
            state.key_checked("good", Ok(KeyCheck::Rejected), None),
            Checked::Nothing
        );
        assert_eq!(state.key.note.as_ref().map(|note| note.1), Some(false));

        assert_eq!(state.save_key(true), KeyStep::Check("good".into()));
        assert_eq!(
            state.key_checked("good", Ok(KeyCheck::Accepted(user(false))), None),
            Checked::Save(
                "good".into(),
                Then::Saved(signed_in(&user(false)), user(false))
            )
        );
        state.key_stored(Then::Saved(signed_in(&user(false)), user(false)));
        assert_eq!(state.user, Some(user(false)));

        state.edit_key(String::new());
        assert_eq!(state.save_key(true), KeyStep::Remove);
    }

    #[test]
    fn an_answer_for_a_key_the_field_moved_on_from_is_let_go() {
        let mut state = NexusState::default();
        state.edit_key("old".into());
        let _ = state.check_key(true);
        state.edit_key("new".into());
        let _ = state.check_key(true);
        let late = state.key_checked("old", Ok(KeyCheck::Accepted(user(true))), Some("old"));
        assert_eq!(late, Checked::Nothing);
        assert!(state.user.is_none(), "an answer about another key");
        assert_eq!(state.key.checking.as_deref(), Some("new"));
        // An accepted key is Zendo's only when it is the stored one.
        let _ = state.key_checked("new", Ok(KeyCheck::Accepted(user(true))), Some("other"));
        assert!(state.user.is_none());
    }

    #[test]
    fn nothing_is_asked_when_it_cannot_run_or_the_field_is_empty() {
        let mut state = NexusState::default();
        assert_eq!(state.check_key(true), KeyStep::Nothing);
        assert!(state.key.note.is_some());
        state.edit_key("k".into());
        assert_eq!(state.check_key(false), KeyStep::Nothing);
        assert_eq!(state.key.checking.as_deref(), Some("k"));
    }

    #[test]
    fn a_bad_link_says_so_and_one_before_the_library_waits() {
        let mut state = NexusState::default();
        assert!(matches!(
            state.link_arrived("nxm://catquest/mods/x", true, true),
            Arrived::Say(note) if note.contains("not a")
        ));
        assert_eq!(
            state.link_arrived("nxm://catquest/mods/1/files/2", false, true),
            Arrived::Wait
        );
        assert_eq!(state.pending.len(), 1);
        assert!(matches!(
            state.link_arrived("nxm://catquest/mods/1/files/2", true, false),
            Arrived::Say(_)
        ));
        assert!(matches!(
            state.link_arrived("nxm://catquest/mods/1/files/2", true, true),
            Arrived::Ask(..)
        ));
    }

    #[test]
    fn a_link_for_no_known_game_asks_which() {
        let mut state = NexusState::default();
        let link = Link::parse("nxm://catquest/mods/1/files/2").unwrap();
        let found = Matched {
            link: link.clone(),
            matched: Vec::new(),
            installed: vec![("epic:cat".to_owned(), "Cat Quest".to_owned())],
            name: "Cat Quest".into(),
        };
        assert_eq!(state.matched(found), Matching::Choose);
        assert!(state.choosing.as_ref().unwrap().all);
        // A game that was not offered is not taken.
        assert!(state.choose("epic:other").is_none());
        assert!(state.choosing.is_none(), "the chooser closes either way");

        let found = Matched {
            link,
            matched: vec![("epic:cat".to_owned(), "Cat Quest".to_owned())],
            installed: Vec::new(),
            name: "Cat Quest".into(),
        };
        assert!(matches!(
            state.matched(found),
            Matching::Download(id, _) if id == "epic:cat"
        ));
    }

    #[test]
    fn one_mods_failed_file_list_keeps_the_others_updates() {
        let mut state = NexusState::default();
        let said = state.updates_found(
            "game:1",
            Ok((
                vec![("mod-a".to_owned(), file(2))],
                vec!["mod-b: offline".to_owned()],
            )),
        );
        assert!(
            state.update_for("game:1", "mod-a").is_some(),
            "the rest stays"
        );
        assert!(
            said.is_some_and(|said| said.contains("mod-b")),
            "the failure is said"
        );
    }

    #[test]
    fn the_update_label_names_the_version_when_there_is_one() {
        let mut newer = file(2);
        assert!(update_label(&newer).contains('2'));
        newer.version = None;
        assert_eq!(update_label(&newer), t!("mods-update"));
    }
}
