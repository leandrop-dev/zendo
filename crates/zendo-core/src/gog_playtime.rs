//! GOG's played time: each session of a GOG game Zendo started itself is
//! sent to GOG as it ends, so the account's played time counts it, as
//! Heroic does (on unless Accounts' GOG card turns it off).
//!
//! GOG takes a session at
//! `POST gameplay.gog.com/games/<game id>/users/<user id>/sessions`, with
//! gogdl's token, as `{"session_date": <start, Unix seconds>, "time":
//! <whole minutes>}`, and answers 201. A session under a minute is not
//! sent. One that cannot be sent (no network, GOG down) waits in
//! `$XDG_DATA_HOME/zendo/gog-playtime.json` and goes with the next one;
//! signing out of GOG drops them, as they belong to that account.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::stores::Store;
use crate::{Game, Launch, Source, paths};

/// Where GOG keeps the sessions played.
const GAMEPLAY: &str = "https://gameplay.gog.com/games";

/// The sessions not sent yet.
const QUEUE_FILE: &str = "gog-playtime.json";

/// One session of a game, as GOG takes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Session {
    /// GOG's id of the game.
    game: String,
    /// When it started, in Unix seconds.
    started: u64,
    /// How long it lasted, in whole minutes.
    time: u64,
}

/// Whether the game's sessions go to GOG: a GOG game Zendo installed and
/// starts through gogdl. One Heroic starts is sent by Heroic.
#[must_use]
pub fn applies(game: &Game) -> bool {
    game.source == Source::Gog
        && matches!(&game.launch, Launch::Engine { runner, .. } if runner == "gogdl")
}

/// Sends the session from `started` to `ended` (Unix seconds), with any
/// left from before; nothing for a session under a minute, or when no one
/// is signed in to GOG.
///
/// # Errors
///
/// When a session could not be sent: it is kept for the next time.
pub fn send(game: &Game, started: u64, ended: u64) -> anyhow::Result<()> {
    let Launch::Engine { app, .. } = &game.launch else {
        return Ok(());
    };
    if !applies(game) || !Config::load_file().is_ok_and(|config| config.gog_playtime) {
        return Ok(());
    }
    let new = played(app, started, ended);
    let file = queue_file().context("no home folder to keep GOG's played time in")?;
    paths::locked(&file, || {
        let mut queue = load(&file);
        queue.extend(new);
        if queue.is_empty() {
            return Ok(());
        }
        let result = match Store::Gog.gog_token() {
            // The sessions are the account's: without one, none is sent.
            Ok(None) => return Ok(()),
            Ok(Some(credentials)) => {
                let agent = crate::http::answering_agent(Duration::from_secs(15));
                send_all(queue, |session| {
                    post(
                        &agent,
                        &credentials.user_id,
                        &credentials.access_token,
                        session,
                    )
                })
            }
            Err(err) => (queue, Some(err)),
        };
        let (left, error) = result;
        save(&file, &left)?;
        match error {
            Some(err) => Err(err.context(format!(
                "GOG's played time ({} kept for next time)",
                minutes_kept(&left)
            ))),
            None => Ok(()),
        }
    })
}

/// The session from `started` to `ended`, in whole minutes; none under a
/// minute.
fn played(game: &str, started: u64, ended: u64) -> Option<Session> {
    let time = ended.saturating_sub(started) / 60;
    (time > 0).then(|| Session {
        game: game.to_owned(),
        started,
        time,
    })
}

/// Forgets the sessions not sent yet, as their account signs out.
///
/// # Errors
///
/// When the file cannot be removed.
pub(crate) fn forget() -> anyhow::Result<()> {
    let Some(file) = queue_file() else {
        return Ok(());
    };
    match fs::remove_file(&file) {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
            Err(err).with_context(|| format!("could not remove {}", file.display()))
        }
        _ => Ok(()),
    }
}

/// Sends each session in turn, keeping those that failed and the last
/// failure.
fn send_all(
    queue: Vec<Session>,
    mut post: impl FnMut(&Session) -> anyhow::Result<()>,
) -> (Vec<Session>, Option<anyhow::Error>) {
    let mut left = Vec::new();
    let mut error = None;
    for session in queue {
        if let Err(err) = post(&session) {
            left.push(session);
            error = Some(err);
        }
    }
    (left, error)
}

/// What GOG is sent for a session.
fn body(session: &Session) -> serde_json::Value {
    serde_json::json!({
        "session_date": session.started,
        "time": session.time,
    })
}

fn post(agent: &ureq::Agent, user: &str, token: &str, session: &Session) -> anyhow::Result<()> {
    let response = agent
        .post(format!("{GAMEPLAY}/{}/users/{user}/sessions", session.game))
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .send(body(session).to_string())
        .context("could not reach GOG")?;
    match response.status().as_u16() {
        200..=299 => Ok(()),
        code => bail!("GOG answered {code}"),
    }
}

/// "1 min", "3 sessions": what waits, for the message.
fn minutes_kept(left: &[Session]) -> String {
    match left {
        [one] => format!("{} min", one.time),
        many => format!("{} sessions", many.len()),
    }
}

fn queue_file() -> Option<PathBuf> {
    paths::data_home().map(|dir| dir.join("zendo").join(QUEUE_FILE))
}

/// The sessions waiting; none when the file is missing or unreadable.
fn load(file: &std::path::Path) -> Vec<Session> {
    fs::read(file)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save(file: &std::path::Path, queue: &[Session]) -> anyhow::Result<()> {
    if queue.is_empty() {
        return match fs::remove_file(file) {
            Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
                Err(err).with_context(|| format!("could not remove {}", file.display()))
            }
            _ => Ok(()),
        };
    }
    paths::write_atomic(file, &serde_json::to_vec_pretty(queue)?)
        .with_context(|| format!("could not write {}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(game: &str, time: u64) -> Session {
        Session {
            game: game.into(),
            started: 1_762_700_000,
            time,
        }
    }

    #[test]
    fn only_gogdls_games_send_their_time() {
        let engine = |runner: &str| Launch::Engine {
            runner: runner.into(),
            app: "1".into(),
        };
        let game = |source, launch| Game::new("gog:1".into(), "Sugar".into(), source, launch);
        assert!(applies(&game(Source::Gog, engine("gogdl"))));
        assert!(!applies(&game(Source::Epic, engine("legendary"))));
        let heroic = Launch::Uri {
            uri: "heroic://launch/gog/1".into(),
        };
        assert!(!applies(&game(Source::Gog, heroic)), "Heroic sends its own");
    }

    #[test]
    fn keeps_the_sessions_that_failed() {
        let queue = vec![session("1", 5), session("2", 7), session("3", 9)];
        let mut sent = Vec::new();
        let (left, error) = send_all(queue, |session| {
            if session.game == "2" {
                bail!("GOG answered 503");
            }
            sent.push(session.game.clone());
            Ok(())
        });
        assert_eq!(sent, ["1", "3"]);
        assert_eq!(left, [session("2", 7)]);
        assert_eq!(
            error.map(|err| err.to_string()).as_deref(),
            Some("GOG answered 503")
        );
        let (left, error) = send_all(vec![session("1", 5)], |_| Ok(()));
        assert!(left.is_empty() && error.is_none());
    }

    #[test]
    fn says_what_waits() {
        assert_eq!(minutes_kept(&[session("1", 42)]), "42 min");
        assert_eq!(
            minutes_kept(&[session("1", 1), session("2", 2)]),
            "2 sessions"
        );
    }

    #[test]
    fn counts_whole_minutes_and_skips_less_than_one() {
        let start = 1_762_700_000;
        assert_eq!(played("1", start, start + 59), None);
        assert_eq!(played("1", start, start + 3_719), Some(session("1", 61)));
        assert_eq!(played("1", start, start - 5), None, "a clock set back");
    }

    #[test]
    fn sends_gog_the_start_and_the_minutes() {
        assert_eq!(
            body(&session("1207658930", 61)).to_string(),
            r#"{"session_date":1762700000,"time":61}"#
        );
    }

    #[test]
    fn keeps_a_waiting_session_with_its_game() {
        let text = serde_json::to_string(&session("1207658930", 61)).unwrap();
        assert_eq!(
            text,
            r#"{"game":"1207658930","started":1762700000,"time":61}"#
        );
    }
}
