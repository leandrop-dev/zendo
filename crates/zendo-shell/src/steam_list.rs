//! Steam's list of owned games, asked with Zendo's own Steam sign-in. Steam's
//! page gives a token that runs out; when it has, or Steam refuses it, the
//! page is asked again for a new one in a window that never shows.

use zendo_core::owned::Owned;
use zendo_core::steam_web::{self, Family, Session, WebAnswer};

use crate::running::now;
use crate::sign_in_window;

/// Steam's list asked with the sign-in.
#[derive(Debug, Clone)]
pub enum SteamFetch {
    Games {
        games: Vec<Owned>,
        /// A new token, when the old one had run out.
        renewed: Option<Session>,
        family: Option<Family>,
        family_error: Option<String>,
    },
    /// Steam's page no longer knows the sign-in.
    SignedOut,
}

/// Steam's list with the sign-in: a new token from Steam's page first when
/// the old one has run out, and again once if Steam refuses it.
///
/// # Errors
///
/// When Steam's page or its Web API cannot be reached.
pub fn fetch_signed_in(session: Session) -> anyhow::Result<SteamFetch> {
    let mut session = session;
    let mut renewed = None;
    if !session.fresh(now()) {
        match sign_in_window::renew_steam()? {
            Some(new) => {
                session = new.clone();
                renewed = Some(new);
            }
            None => return Ok(SteamFetch::SignedOut),
        }
    }
    let mut answer = steam_web::ask(&session)?;
    if matches!(answer, WebAnswer::Rejected) && renewed.is_none() {
        match sign_in_window::renew_steam()? {
            Some(new) => {
                answer = steam_web::ask(&new)?;
                renewed = Some(new);
            }
            None => return Ok(SteamFetch::SignedOut),
        }
    }
    match answer {
        WebAnswer::Games {
            games,
            family,
            family_error,
        } => Ok(SteamFetch::Games {
            games: steam_web::keep(games)?,
            renewed,
            family,
            family_error,
        }),
        WebAnswer::Rejected => Ok(SteamFetch::SignedOut),
    }
}
