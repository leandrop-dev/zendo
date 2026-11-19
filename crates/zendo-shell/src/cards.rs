//! What a game's page draws from what the stores and the sites say:
//! `ProtonDB`'s pill, Steam's store page (its about line and the facts only
//! Steam has), the system requirements and the achievements' rows. The words
//! and the choices are one, so both windows agree; asking the network and
//! keeping the answers stay with each window.

use std::path::PathBuf;

use zendo_core::Game;
use zendo_core::achievements::{Achievement, Achievements};
use zendo_core::owned::{Owned, StoreDetails};
use zendo_core::protondb::Rating;
use zendo_core::requirements::{From, Requirements, System};
use zendo_core::steam_facts::Facts;

use crate::{format, t};

/// How many achievements show at first, and how many more Show more adds.
pub const SHOWN: usize = 10;

/// How many of Steam's features the details card names before "+N more".
const FEATURES_SHOWN: usize = 6;

/// A badge on the header that opens a page: `ProtonDB`'s rating.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pill {
    pub label: String,
    /// 0 takes `tint`; 1, 2 and 3 take the theme's good, bad and dim.
    pub tone: i32,
    /// The tier's own colour, for tone 0.
    pub tint: Option<(u8, u8, u8)>,
    /// What it says on hover.
    pub tip: String,
    pub url: String,
}

/// A line of a card: a label and its value, and the link the value opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub label: String,
    pub value: String,
    pub url: Option<String>,
}

/// The requirements card: where the answer came from, whether it has both
/// levels, and a line per thing asked for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequirementsView {
    pub source: String,
    pub both: bool,
    pub rows: Vec<RequirementRow>,
}

/// One thing asked for: "Memory", with what each level asks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementRow {
    pub label: String,
    pub minimum: String,
    pub recommended: String,
}

/// One achievement, as its row draws it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AchievementRow {
    /// Its picture, once downloaded.
    pub icon: Option<PathBuf>,
    pub name: String,
    pub description: String,
    /// When it was unlocked or how rare it is.
    pub side: String,
    pub unlocked: bool,
}

/// The pill for a rating: Native for a game with a Linux build, else the
/// reports' tier.
#[must_use]
pub fn rating_pill(rating: &Rating) -> Pill {
    let (label, tone, tint, tip) = match &rating.tier {
        _ if rating.native => (
            t!("protondb-native"),
            1,
            None,
            t!("protondb-native-about", reports = reports(rating)),
        ),
        Some(tier) => {
            let own = tier_color(tier);
            (
                format!("PROTONDB {}", tier.to_uppercase()),
                if own.is_some() {
                    0
                } else if tier == "borked" {
                    2
                } else {
                    3
                },
                own,
                t!("protondb-tier-about", reports = reports(rating)),
            )
        }
        None => (t!("protondb-none"), 3, None, t!("protondb-none-about")),
    };
    Pill {
        label,
        tone,
        tint,
        tip,
        url: rating.url(),
    }
}

/// The pill while `ProtonDB` is being asked.
#[must_use]
pub fn asking_pill() -> Pill {
    Pill {
        label: "PROTONDB …".to_owned(),
        tone: 3,
        tint: None,
        tip: String::new(),
        url: String::new(),
    }
}

/// The colour of a `ProtonDB` tier, or `None` for the theme's own.
fn tier_color(tier: &str) -> Option<(u8, u8, u8)> {
    match tier {
        "platinum" => Some((0xb4, 0xc7, 0xdc)),
        "gold" => Some((0xcf, 0xb5, 0x3b)),
        "silver" => Some((0xa6, 0xa6, 0xa6)),
        "bronze" => Some((0xcd, 0x7f, 0x32)),
        _ => None,
    }
}

fn title_case(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// What the reports say, for the pill's words.
fn reports(rating: &Rating) -> String {
    match (&rating.tier, rating.reports) {
        (None, _) => t!("protondb-no-reports-yet"),
        (Some(tier), count) => t!("protondb-reports", tier = title_case(tier), count = count),
    }
}

/// Steam's features of the game, with its controller support when the
/// features do not already say it.
#[must_use]
pub fn features_line(details: &StoreDetails) -> String {
    let mut features = details.features.clone();
    let controller = match details.controller.as_deref() {
        Some("full") => Some(t!("fact-controller-full")),
        Some("partial") => Some(t!("fact-controller-partial")),
        _ => None,
    };
    if let Some(controller) = controller
        && !features
            .iter()
            .any(|feature| feature.eq_ignore_ascii_case(&controller))
    {
        features.push(controller);
    }
    // Steam lists up to thirty; the first few say what the game is.
    let more = features.len().saturating_sub(FEATURES_SHOWN);
    features.truncate(FEATURES_SHOWN);
    let mut line = features.join(", ");
    if more > 0 {
        line.push_str(&t!("fact-features-more", count = more.to_string()));
    }
    line
}

/// Rows for the details card from Steam's store: the developer, publisher,
/// release date and genres, then the features and the Metacritic score. What
/// a game's own store already said (`owned`) is not said twice.
#[must_use]
pub fn steam_rows(facts: &Facts, game: &Game, owned: Option<&Owned>) -> Vec<Row> {
    let details = &facts.details;
    let said = |own: Option<&Option<String>>| own.is_some_and(Option::is_some);
    let mut rows = Vec::new();
    if let Some(developer) = &details.developer
        && game.details.developer.is_none()
        && !said(owned.map(|owned| &owned.developer))
    {
        rows.push(row(t!("fact-developer"), developer.clone()));
    }
    if let Some(publisher) = &details.publisher
        && !said(owned.map(|owned| &owned.publisher))
    {
        rows.push(row(t!("fact-publisher"), publisher.clone()));
    }
    if let Some(day) = &details.released
        && !said(owned.map(|owned| &owned.released))
    {
        rows.push(row(t!("fact-released"), day.clone()));
    }
    if !details.genres.is_empty() && owned.is_none_or(|owned| owned.genres.is_empty()) {
        rows.push(row(t!("fact-genres"), details.genres.join(", ")));
    }
    let features = features_line(details);
    if !features.is_empty() {
        rows.push(row(t!("fact-features"), features));
    }
    if let Some(found) = &details.metacritic {
        rows.push(Row {
            url: Some(found.url.clone()),
            ..row(t!("fact-metacritic"), format!("{} / 100", found.score))
        });
    }
    rows
}

/// The requirements card's data.
#[must_use]
pub fn requirements_view(found: &Requirements) -> RequirementsView {
    let system = match found.system {
        System::Windows => "Windows",
        System::Linux => "Linux",
    };
    let source = match found.from {
        From::Gog => t!("requirements-from-gog", system = system),
        From::Epic => t!("requirements-from-epic", system = system),
        From::Steam => t!("requirements-from-steam", system = system),
    };
    RequirementsView {
        source,
        both: found.has_recommended(),
        rows: found
            .rows
            .iter()
            .map(|row| RequirementRow {
                label: row.label.clone(),
                minimum: row.minimum.clone().unwrap_or_default(),
                recommended: row.recommended.clone().unwrap_or_default(),
            })
            .collect(),
    }
}

/// The achievements card's first line and the share of them unlocked, when
/// the store said which ones the player has.
#[must_use]
pub fn achievements_summary(found: &Achievements, id: &str) -> (String, Option<f32>) {
    let total = found.list.len();
    if found.unlocks_known {
        let unlocked = found.unlocked();
        #[expect(
            clippy::cast_precision_loss,
            reason = "counts of achievements, far below f32's exact range"
        )]
        let share = unlocked as f32 / total.max(1) as f32;
        (
            t!("achievements-unlocked", unlocked = unlocked, total = total),
            Some(share),
        )
    } else {
        let why = if id.starts_with("steam:") {
            t!("achievements-unknown-steam", total = total)
        } else {
            t!("achievements-unknown", total = total)
        };
        (why, None)
    }
}

/// The achievements' rows, the first `shown` of them: the icon's path is
/// where it was downloaded; a locked hidden one keeps its name back.
#[must_use]
pub fn achievement_rows(found: &Achievements, shown: usize, at: u64) -> Vec<AchievementRow> {
    let shown = shown.min(found.list.len());
    found.list[..shown]
        .iter()
        .map(|achievement| achievement_row(achievement, at))
        .collect()
}

/// What Show more says, when there are more to show.
#[must_use]
pub fn more_label(found: &Achievements, shown: usize) -> Option<String> {
    let left = found.list.len().saturating_sub(shown.min(found.list.len()));
    (left > 0).then(|| t!("achievements-more", count = left))
}

/// A share of players: one decimal under 10 %, whole numbers above.
#[must_use]
pub fn rarity_text(percent: f32) -> String {
    if percent < 10.0 {
        format!("{percent:.1}")
    } else {
        format!("{percent:.0}")
    }
}

fn achievement_row(achievement: &Achievement, at: u64) -> AchievementRow {
    let kept_back = achievement.hidden && achievement.unlocked.is_none();
    let (name, description) = if kept_back {
        (t!("achievements-hidden"), t!("achievements-hidden-about"))
    } else {
        (achievement.name.clone(), achievement.description.clone())
    };
    let side = match (achievement.unlocked, achievement.rarity) {
        (Some(when), _) if when > 0 => {
            t!("achievements-unlocked-when", when = format::ago(when, at))
        }
        (Some(_), _) => t!("achievements-unlocked-plain"),
        (None, Some(rarity)) => t!("achievements-rarity", percent = rarity_text(rarity)),
        (None, None) => String::new(),
    };
    AchievementRow {
        icon: achievement.icon.clone(),
        name,
        description,
        side,
        unlocked: achievement.unlocked.is_some(),
    }
}

fn row(label: String, value: String) -> Row {
    Row {
        label,
        value,
        url: None,
    }
}

#[cfg(test)]
mod tests {
    use zendo_core::owned::Metacritic;
    use zendo_core::{Launch, Source};

    use super::*;

    fn details() -> StoreDetails {
        StoreDetails {
            features: vec!["Single-player".into(), "Co-op".into()],
            controller: Some("full".into()),
            metacritic: Some(Metacritic {
                score: 95,
                url: "https://www.metacritic.com/game/pc/portal-2".into(),
            }),
            ..StoreDetails::default()
        }
    }

    fn game() -> Game {
        Game::new(
            "steam:400".into(),
            "Portal".into(),
            Source::Steam,
            Launch::Command {
                program: "portal".into(),
                args: Vec::new(),
                dir: None,
            },
        )
    }

    fn achievements() -> Achievements {
        let item = |name: &str, unlocked: Option<u64>, hidden: bool| Achievement {
            name: name.into(),
            description: format!("{name} it"),
            hidden,
            unlocked,
            rarity: None,
            icon: None,
        };
        Achievements {
            list: vec![
                item("First", Some(1_700_000_000), false),
                item("Second", None, false),
                item("Secret", None, true),
            ],
            unlocks_known: true,
        }
    }

    #[test]
    fn names_the_controller_once_and_counts_the_rest() {
        let mut details = details();
        assert_eq!(
            features_line(&details),
            format!("Single-player, Co-op, {}", t!("fact-controller-full"))
        );
        details.features.push(t!("fact-controller-full"));
        assert_eq!(features_line(&details).matches("ontroller").count(), 1);
        details.controller = None;
        details.features = (1..=9).map(|n| format!("F{n}")).collect();
        let line = features_line(&details);
        assert!(line.starts_with("F1, F2, F3, F4, F5, F6"));
        assert!(!line.contains("F7"));
        assert!(line.ends_with(&t!("fact-features-more", count = "3".to_owned())));
    }

    #[test]
    fn a_pill_says_native_or_the_tier() {
        let rating = Rating {
            app: 435_790,
            tier: Some("gold".into()),
            reports: 37,
            native: false,
        };
        let pill = rating_pill(&rating);
        assert_eq!(pill.label, "PROTONDB GOLD");
        assert_eq!(pill.tone, 0);
        assert_eq!(pill.tint, Some((0xcf, 0xb5, 0x3b)));
        assert_eq!(reports(&rating), "Proton: Gold from 37 reports");
        let native = Rating {
            native: true,
            ..rating
        };
        assert_eq!(rating_pill(&native).tone, 1);
        let borked = Rating {
            app: 1,
            tier: Some("borked".into()),
            reports: 2,
            native: false,
        };
        assert_eq!(rating_pill(&borked).tone, 2);
        let pending = Rating {
            app: 1,
            tier: Some("pending".into()),
            reports: 0,
            native: false,
        };
        assert_eq!(rating_pill(&pending).tone, 3);
    }

    #[test]
    fn rarity_has_a_decimal_under_ten_percent() {
        assert_eq!(rarity_text(3.24), "3.2");
        assert_eq!(rarity_text(42.4), "42");
    }

    #[test]
    fn steam_rows_name_what_the_game_does_not() {
        let mut details = details();
        details.developer = Some("Valve".into());
        details.publisher = Some("Valve".into());
        details.released = Some("18 Oct, 2007".into());
        details.genres = vec!["Action".into()];
        let facts = Facts {
            details,
            shots: Vec::new(),
        };
        let mut game = game();
        game.details.developer = Some("Valve".into());
        let labels: Vec<String> = steam_rows(&facts, &game, None)
            .into_iter()
            .map(|row| row.label)
            .collect();
        assert_eq!(
            labels,
            [
                t!("fact-publisher"),
                t!("fact-released"),
                t!("fact-genres"),
                t!("fact-features"),
                t!("fact-metacritic"),
            ],
            "the developer is already under the title"
        );
        let rows = steam_rows(&facts, &game, None);
        let metacritic = rows.last().expect("a row");
        assert_eq!(metacritic.value, "95 / 100");
        assert!(metacritic.url.is_some());
    }

    #[test]
    fn the_achievements_say_which_are_unlocked_and_keep_secrets() {
        let found = achievements();
        let (summary, share) = achievements_summary(&found, "steam:400");
        assert_eq!(summary, "1 of 3 unlocked");
        assert_eq!(share, Some(1.0 / 3.0));
        let rows = achievement_rows(&found, SHOWN, 0);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1].side, "");
        assert_eq!(rows[2].name, t!("achievements-hidden"));
        assert!(!rows[2].unlocked);
        assert!(more_label(&found, 2).is_some());
        assert_eq!(more_label(&found, SHOWN), None);
    }
}
