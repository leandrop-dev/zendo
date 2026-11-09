//! Steam through the user's sign-in on Steam's own web page, in Zendo's
//! sign-in window: Steam's store page gives a signed-in user a Web API
//! access token (`webapi_token`), as Playnite reads it. With it, Steam's Web
//! API lists the games the account owns and those its Steam family shares.
//!
//! Zendo never sees the password: the user signs in on Steam's page, with
//! the password and Steam Guard or the Steam app's QR code. The token lasts
//! a day or so; Steam's page, which remembers the sign-in, gives a new one
//! ([`Session`] says when one is due).

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, bail};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};

use crate::owned::{self, Owned, Shop, SteamAnswer};
use crate::paths;

/// The page the sign-in starts and ends on: Steam's store, which carries
/// the token once signed in.
pub const STORE_PAGE: &str = "https://store.steampowered.com/explore/";
/// Steam's sign-in page, back to [`STORE_PAGE`] after.
pub const SIGN_IN_PAGE: &str = "https://store.steampowered.com/login/?redir=explore%2F";

const FAMILY_GROUP: &str =
    "https://api.steampowered.com/IFamilyGroupsService/GetFamilyGroupForUser/v1/";
const FAMILY_APPS: &str =
    "https://api.steampowered.com/IFamilyGroupsService/GetSharedLibraryApps/v1/";
const FAMILY_DETAILS: &str = "https://api.steampowered.com/IFamilyGroupsService/GetFamilyGroup/v1/";

/// The folder where the sign-in window keeps Steam's cookies, so Steam's
/// page remembers the sign-in: `$XDG_DATA_HOME/zendo/steam-web`.
#[must_use]
pub fn profile_dir() -> Option<PathBuf> {
    paths::data_home().map(|data| data.join("zendo/steam-web"))
}

/// Makes the cookies' folder, readable by its owner only.
///
/// # Errors
///
/// When it cannot be made.
pub fn make_profile_dir(dir: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(dir).with_context(|| format!("could not make {}", dir.display()))?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("could not make {} private", dir.display()))
}

/// Signs out of Steam's page: its cookies go.
///
/// # Errors
///
/// When the folder is there but cannot be removed.
pub fn forget_profile() -> anyhow::Result<()> {
    let Some(dir) = profile_dir() else {
        return Ok(());
    };
    match fs::remove_dir_all(&dir) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("could not remove {}", dir.display())),
    }
}

/// A token given that much before it ends is asked again.
const MARGIN: u64 = 10 * 60;

/// A signed-in Steam account: its Steam ID and the Web API token Steam's
/// page gave. Kept in the keyring as JSON; its `Debug` leaves the token
/// out.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub steamid: String,
    pub token: String,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("steamid", &self.steamid)
            .finish_non_exhaustive()
    }
}

impl Session {
    /// Reads what the keyring keeps, or what the sign-in window writes.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let session: Self = serde_json::from_str(text.trim()).ok()?;
        let valid = session.steamid.len() == 17
            && session.steamid.bytes().all(|b| b.is_ascii_digit())
            && !session.token.is_empty();
        valid.then_some(session)
    }

    /// As the keyring keeps it.
    #[must_use]
    pub fn to_text(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// When the token ends, in seconds since 1970, as it says itself (a
    /// JWT's `exp`); `None` when it does not say.
    #[must_use]
    pub fn expires(&self) -> Option<u64> {
        #[derive(Deserialize)]
        struct Claims {
            exp: Option<u64>,
        }
        let payload = self.token.split('.').nth(1)?;
        let bytes = URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
        serde_json::from_slice::<Claims>(&bytes).ok()?.exp
    }

    /// Whether the token is good for a while yet; one that does not say
    /// when it ends is tried, and asked again if Steam refuses it.
    #[must_use]
    pub fn fresh(&self, now: u64) -> bool {
        self.expires()
            .is_none_or(|expires| expires > now.saturating_add(MARGIN))
    }
}

/// What Steam answered to a session.
#[derive(Debug)]
pub enum WebAnswer {
    /// The games owned and, after them, those the family shares; with why
    /// the family's could not be listed, when they could not.
    Games {
        games: Vec<Owned>,
        /// The account's Steam family, when it is in one.
        family: Option<Family>,
        family_error: Option<String>,
    },
    /// Steam does not take the token: it ended, or the user signed out.
    Rejected,
}

/// A Steam family, as its members see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Family {
    pub name: String,
    /// How many accounts are in it, the user's among them.
    pub members: usize,
}

/// Asks Steam for the games the account owns and those its family shares.
///
/// # Errors
///
/// When Steam cannot be reached or answers oddly.
pub fn ask(session: &Session) -> anyhow::Result<WebAnswer> {
    let mut games = match owned::ask_owned(&session.token, &session.steamid)? {
        SteamAnswer::Games(games) => games,
        SteamAnswer::Nothing => Vec::new(),
        SteamAnswer::Rejected => return Ok(WebAnswer::Rejected),
    };
    let (family, family_error) = match family(session) {
        Ok(None) => (None, None),
        Ok(Some((family, shared))) => {
            let owned: HashSet<String> = games.iter().map(|game| game.id.clone()).collect();
            games.extend(shared.into_iter().filter(|game| !owned.contains(&game.id)));
            (family, None)
        }
        Err(err) => (None, Some(format!("{err:#}"))),
    };
    Ok(WebAnswer::Games {
        games,
        family,
        family_error,
    })
}

/// Keeps the list as the Store's Steam list and returns it.
///
/// # Errors
///
/// When the cache cannot be written.
pub fn keep(games: Vec<Owned>) -> anyhow::Result<Vec<Owned>> {
    owned::keep(Shop::Steam, games)
}

/// The account's Steam family and the games it shares with it; `None` when
/// it is in no family. The family's name is left out when only it could
/// not be read.
fn family(session: &Session) -> anyhow::Result<Option<(Option<Family>, Vec<Owned>)>> {
    let agent = crate::http::agent(Duration::from_secs(30));
    let read = |response: Result<ureq::http::Response<ureq::Body>, ureq::Error>| match response {
        Ok(response) => Ok(response.into_body().read_to_string()?),
        Err(ureq::Error::StatusCode(code)) => bail!("Steam's family answered {code}"),
        Err(err) => Err(err).context("could not reach Steam's family"),
    };
    let group = read(
        agent
            .get(FAMILY_GROUP)
            .query("access_token", &session.token)
            .query("steamid", &session.steamid)
            .call(),
    )?;
    let Some(group) = parse_group(&group)? else {
        return Ok(None);
    };
    let apps = read(
        agent
            .get(FAMILY_APPS)
            .query("access_token", &session.token)
            .query("steamid", &session.steamid)
            .query("family_groupid", &group)
            .query("include_own", "false")
            .query("include_excluded", "false")
            .query("include_non_games", "false")
            .query("language", "english")
            .call(),
    )?;
    let shared = parse_shared(&apps, &session.steamid)?;
    let details = read(
        agent
            .get(FAMILY_DETAILS)
            .query("access_token", &session.token)
            .query("family_groupid", &group)
            .call(),
    );
    let family = details.ok().and_then(|text| parse_family(&text));
    Ok(Some((family, shared)))
}

/// The family's name and how many are in it.
fn parse_family(text: &str) -> Option<Family> {
    #[derive(Deserialize)]
    struct Reply {
        response: Group,
    }
    #[derive(Deserialize)]
    struct Group {
        #[serde(default)]
        name: String,
        #[serde(default)]
        members: Vec<serde::de::IgnoredAny>,
    }
    let group = serde_json::from_str::<Reply>(text).ok()?.response;
    Some(Family {
        name: group.name.trim().to_owned(),
        members: group.members.len(),
    })
}

/// The family group's id; `None` when the account is in none.
fn parse_group(text: &str) -> anyhow::Result<Option<String>> {
    #[derive(Deserialize)]
    struct Reply {
        response: Group,
    }
    #[derive(Deserialize)]
    struct Group {
        family_groupid: Option<String>,
        #[serde(default)]
        is_not_member_of_any_group: bool,
    }
    let reply: Reply = serde_json::from_str(text).context("Steam's family group: not JSON")?;
    let group = reply.response;
    Ok(group
        .family_groupid
        .filter(|id| !id.is_empty() && id != "0" && !group.is_not_member_of_any_group))
}

/// The family's games the account can play, those it owns itself left out,
/// marked as shared.
fn parse_shared(text: &str, steamid: &str) -> anyhow::Result<Vec<Owned>> {
    #[derive(Deserialize)]
    struct Reply {
        response: Library,
    }
    #[derive(Deserialize)]
    struct Library {
        #[serde(default)]
        apps: Vec<App>,
    }
    #[derive(Deserialize)]
    struct App {
        appid: u64,
        name: Option<String>,
        #[serde(default)]
        owner_steamids: Vec<String>,
        #[serde(default)]
        exclude_reason: u32,
        /// The account's own minutes in it.
        #[serde(default)]
        rt_playtime: u64,
        /// 0 when never played.
        #[serde(default)]
        rt_last_played: u64,
    }
    let reply: Reply = serde_json::from_str(text).context("Steam's family library: not JSON")?;
    Ok(reply
        .response
        .apps
        .into_iter()
        .filter(|app| app.exclude_reason == 0)
        .filter(|app| !app.owner_steamids.iter().any(|owner| owner == steamid))
        .map(|app| {
            let mut game = owned::steam_game(app.appid, app.name, app.rt_playtime);
            game.shared = true;
            game.last_played = Some(app.rt_last_played).filter(|time| *time > 0);
            game
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: &str = "76561198000000001";

    fn token_with(claims: &str) -> String {
        format!(
            "{}.{}.signature",
            URL_SAFE_NO_PAD.encode(r#"{"typ":"JWT"}"#),
            URL_SAFE_NO_PAD.encode(claims)
        )
    }

    #[test]
    fn reads_a_session_and_when_it_ends() {
        let token = token_with(r#"{"sub":"76561198000000001","exp":2000000000}"#);
        let text = format!(r#"{{"steamid":"{ME}","token":"{token}"}}"#);
        let session = Session::parse(&text).unwrap();
        assert_eq!(session.expires(), Some(2_000_000_000));
        assert!(session.fresh(1_999_000_000));
        assert!(!session.fresh(1_999_999_500));
        assert_eq!(Session::parse(&session.to_text()), Some(session.clone()));
        assert!(!format!("{session:?}").contains(&token));
        // A token that does not say when it ends is tried.
        let opaque = Session {
            steamid: ME.into(),
            token: "opaque".into(),
        };
        assert!(opaque.fresh(u64::MAX));
        assert_eq!(Session::parse(r#"{"steamid":"1","token":"x"}"#), None);
        assert_eq!(Session::parse("not json"), None);
    }

    #[test]
    fn reads_the_family_group() {
        let group =
            r#"{"response":{"family_groupid":"20782235","is_not_member_of_any_group":false}}"#;
        assert_eq!(parse_group(group).unwrap().as_deref(), Some("20782235"));
        let none = r#"{"response":{"is_not_member_of_any_group":true}}"#;
        assert_eq!(parse_group(none).unwrap(), None);
        let details = r#"{"response":{"name":" The Family ","members":[
            {"steamid":"76561198000000001","role":1},{"steamid":"76561198000000002","role":2}
        ],"free_spots":4}}"#;
        assert_eq!(
            parse_family(details),
            Some(Family {
                name: "The Family".into(),
                members: 2
            })
        );
    }

    #[test]
    fn lists_the_games_the_family_shares() {
        let library = r#"{"response":{"apps":[
            {"appid":4704690,"name":"MECCHA CHAMELEON","owner_steamids":["76561198000000002"],"exclude_reason":0,"rt_playtime":42,"rt_last_played":1700000000},
            {"appid":620,"name":"Portal 2","owner_steamids":["76561198000000001","76561198000000002"],"exclude_reason":0},
            {"appid":10,"name":"Kept back","owner_steamids":["76561198000000002"],"exclude_reason":2}
        ],"owner_steamid":"76561198000000001"}}"#;
        let shared = parse_shared(library, ME).unwrap();
        assert_eq!(shared.len(), 1);
        assert_eq!(shared[0].id, "steam:4704690");
        assert_eq!(shared[0].title, "MECCHA CHAMELEON");
        assert_eq!(shared[0].minutes, Some(42));
        assert_eq!(shared[0].last_played, Some(1_700_000_000));
        assert!(shared[0].shared);
        assert_eq!(parse_shared(r#"{"response":{}}"#, ME).unwrap(), Vec::new());
    }
}
