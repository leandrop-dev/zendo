//! What a store says about a game, as rows for the details card: the sizes
//! and latest version its engine gives, then what its list records.

use zendo_core::Size;
use zendo_core::info::GameInfo;
use zendo_core::owned::Owned;

use crate::t;

/// What the stores said about one game: its size and latest version, or its
/// details from Steam's store. Asked once a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InfoState {
    Asking,
    Got(GameInfo),
    Failed(String),
}

/// What the store says about a game: sizes and the latest version asked of
/// its engine, then what its list records. `installed` leaves out the size
/// on disk, which the install's own record gives.
#[must_use]
pub fn facts(
    owned: Option<&Owned>,
    info: Option<&InfoState>,
    installed: bool,
) -> Vec<(String, String)> {
    let mut facts = Vec::new();
    match info {
        // Steam's store is asked for the page's details, not sizes.
        Some(InfoState::Asking) if owned.is_some_and(|owned| owned.store().is_none()) => {
            facts.push((t!("fact-store-details"), t!("fact-asking")));
        }
        Some(InfoState::Asking) => facts.push((t!("fact-download-size"), t!("fact-asking"))),
        Some(InfoState::Got(info)) => {
            if let Some(bytes) = info.download_size {
                facts.push((t!("fact-download-size"), Size::Bytes(bytes).to_string()));
            }
            if let Some(bytes) = info.install_size.filter(|_| !installed) {
                facts.push((t!("fact-size-on-disk"), Size::Bytes(bytes).to_string()));
            }
            if let Some(version) = &info.version {
                facts.push((t!("fact-latest-version"), version.clone()));
            }
            if let Some(day) = &info.updated {
                facts.push((t!("fact-latest-build"), day.clone()));
            }
        }
        Some(InfoState::Failed(err)) => {
            facts.push((
                t!("fact-store-details"),
                t!("fact-store-failed", error = err.as_str()),
            ));
        }
        None => {}
    }
    if let Some(owned) = owned {
        if let Some(sizes) = owned.sizes {
            facts.push((
                t!("fact-download-size"),
                Size::Bytes(sizes.download).to_string(),
            ));
            if !installed {
                facts.push((
                    t!("fact-size-on-disk"),
                    Size::Bytes(sizes.install).to_string(),
                ));
            }
        } else if let Some(bytes) = owned.space.filter(|_| !installed) {
            facts.push((t!("fact-space-needed"), Size::Bytes(bytes).to_string()));
        }
        if let Some(publisher) = &owned.publisher {
            facts.push((t!("fact-publisher"), publisher.clone()));
        }
        if let Some(day) = &owned.released {
            facts.push((t!("fact-released"), day.clone()));
        }
        if !owned.genres.is_empty() {
            facts.push((t!("fact-genres"), owned.genres.join(", ")));
        }
    }
    facts
}

#[cfg(test)]
mod tests {
    use zendo_core::owned::Shop;

    use super::*;

    #[test]
    fn says_sizes_and_version_once_asked_and_why_when_it_failed() {
        let mut game = Owned::new(Shop::Epic, "a", "A".into());
        game.publisher = Some("Maker".into());
        let info = InfoState::Got(GameInfo {
            download_size: Some(2_000_000_000),
            install_size: Some(5_000_000_000),
            version: Some("1.2".into()),
            ..GameInfo::default()
        });
        let labels = |rows: Vec<(String, String)>| -> Vec<String> {
            rows.into_iter().map(|(label, _)| label).collect()
        };
        assert_eq!(
            labels(facts(Some(&game), Some(&info), false)),
            [
                t!("fact-download-size"),
                t!("fact-size-on-disk"),
                t!("fact-latest-version"),
                t!("fact-publisher"),
            ]
        );
        assert_eq!(
            labels(facts(Some(&game), Some(&info), true)),
            [
                t!("fact-download-size"),
                t!("fact-latest-version"),
                t!("fact-publisher"),
            ],
            "an installed game has its own size on disk"
        );
        let failed = InfoState::Failed("offline".into());
        assert_eq!(
            facts(None, Some(&failed), false),
            [(
                t!("fact-store-details"),
                t!("fact-store-failed", error = "offline")
            )]
        );
        let steam = Owned::new(Shop::Steam, "620", "Portal 2".into());
        assert_eq!(
            facts(Some(&steam), Some(&InfoState::Asking), false)[0].0,
            t!("fact-store-details")
        );
    }
}
