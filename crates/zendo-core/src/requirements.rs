//! What a game asks of the computer, minimum and recommended, from the
//! game's own store when it says, else from Steam's store page for the
//! Steam app id found for it (as for `ProtonDB`):
//!
//! - GOG: `api.gog.com/v2/games/<id>`, `_embedded.supportedOperatingSystems`,
//!   each with `systemRequirements` per level (`minimum`, `recommended`) of
//!   `requirements` with a `name` ("Processor:") and a `description`.
//!   Older games often have none.
//! - Epic: the store page's slug from Epic's launcher GraphQL
//!   (`catalogNs(namespace) { mappings(pageType: "productHome") }`), then
//!   `store-content.ak.epicgames.com/api/en-US/content/products/<slug>`,
//!   the `productHome` page's `requirements.systems` with `details` of
//!   `title`, `minimum` and `recommended`, as Heroic reads it. Newer store
//!   pages (a slug ending in a code, `botany-manor-e7c456`) answer nothing.
//! - Steam: `appdetails`' `pc_requirements` and `linux_requirements`, each
//!   `minimum` and `recommended` HTML, lines of "Label: value"; `[]` when
//!   the store lists none.
//!
//! All in English, as Zendo's other store details. Answers are kept in
//! `$XDG_CACHE_HOME/zendo/requirements.json` and asked again after a week.

use std::collections::HashMap;
use std::fs;
use std::time::Duration;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{owned, paths, store_tasks, time};

const GOG_GAME: &str = "https://api.gog.com/v2/games";
const EPIC_GRAPHQL: &str = "https://launcher.store.epicgames.com/graphql";
/// Epic's GraphQL answers the launcher; Heroic sends its User-Agent too.
const EPIC_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) EpicGamesLauncher";
const EPIC_CONTENT: &str = "https://store-content.ak.epicgames.com/api/en-US/content/products";
const STEAM_DETAILS: &str = "https://store.steampowered.com/api/appdetails";
const WEEK: u64 = 7 * 24 * 60 * 60;

/// The system the requirements are for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum System {
    Windows,
    Linux,
}

/// Where the requirements came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum From {
    Gog,
    Epic,
    Steam,
}

/// One thing asked for: "Memory", with what each level asks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    /// Empty for a line with no label ("Requires a 64-bit processor").
    pub label: String,
    pub minimum: Option<String>,
    pub recommended: Option<String>,
}

/// A game's requirements for one system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Requirements {
    pub system: System,
    pub from: From,
    pub rows: Vec<Row>,
}

impl Requirements {
    /// Whether any row has a recommended value.
    #[must_use]
    pub fn has_recommended(&self) -> bool {
        self.rows.iter().any(|row| row.recommended.is_some())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Kept {
    /// Unix seconds.
    asked: u64,
    requirements: Option<Requirements>,
}

/// The game's requirements for the system, by its Zendo id and the Steam
/// app id found for it, from the cache or asked anew: `None` when no store
/// lists them. A game with none for Linux gets Windows'. It blocks on the
/// network; the window calls it on a thread.
///
/// # Errors
///
/// When a store cannot be reached or answers oddly; nothing is kept then,
/// so the next look asks again.
pub fn for_game(
    id: &str,
    steam_app: Option<u32>,
    system: System,
) -> anyhow::Result<Option<Requirements>> {
    let file = paths::cache_home().map(|dir| dir.join("zendo/requirements.json"));
    let key = format!("{id}/{system:?}");
    let mut kept: HashMap<String, Kept> = file
        .as_ref()
        .and_then(|file| fs::read(file).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let now = time::now();
    if let Some(entry) = kept.get(&key)
        && now.saturating_sub(entry.asked) < WEEK
    {
        return Ok(entry.requirements.clone());
    }
    let mut found = None;
    for wanted in [system, System::Windows] {
        found = ask(id, steam_app, wanted)?;
        if found.is_some() || wanted == System::Windows {
            break;
        }
    }
    kept.insert(
        key,
        Kept {
            asked: now,
            requirements: found.clone(),
        },
    );
    if let Some(file) = file
        && let Ok(json) = serde_json::to_vec(&kept)
    {
        let _ = paths::write_atomic(&file, &json);
    }
    Ok(found)
}

/// The game's own store first, then Steam's.
fn ask(id: &str, steam_app: Option<u32>, system: System) -> anyhow::Result<Option<Requirements>> {
    let agent = crate::http::answering_agent(Duration::from_secs(15));
    let (store, app) = id.split_once(':').unwrap_or(("", id));
    let own = match store {
        "gog" => ask_gog(&agent, app, system)?.map(|rows| (From::Gog, rows)),
        "epic" => ask_epic(&agent, app, system)?.map(|rows| (From::Epic, rows)),
        _ => None,
    };
    if let Some((from, rows)) = own {
        return Ok(Some(Requirements { system, from, rows }));
    }
    let steam_app = if store == "steam" {
        app.parse().ok()
    } else {
        steam_app
    };
    let Some(steam_app) = steam_app else {
        return Ok(None);
    };
    Ok(
        ask_steam(&agent, steam_app, system)?.map(|rows| Requirements {
            system,
            from: From::Steam,
            rows,
        }),
    )
}

fn get_text(
    request: ureq::RequestBuilder<ureq::typestate::WithoutBody>,
) -> anyhow::Result<Option<String>> {
    let mut answer = request.call()?;
    if !answer.status().is_success() {
        return Ok(None);
    }
    Ok(Some(answer.body_mut().read_to_string()?))
}

fn ask_gog(agent: &ureq::Agent, product: &str, system: System) -> anyhow::Result<Option<Vec<Row>>> {
    let url = format!("{GOG_GAME}/{product}");
    let text = get_text(agent.get(&url).query("locale", "en-US"))
        .context("could not ask GOG for the requirements")?;
    Ok(text.and_then(|text| parse_gog(&text, system)))
}

fn parse_gog(text: &str, system: System) -> Option<Vec<Row>> {
    let game: Value = serde_json::from_str(text).ok()?;
    let name = match system {
        System::Windows => "windows",
        System::Linux => "linux",
    };
    let levels = game
        .pointer("/_embedded/supportedOperatingSystems")?
        .as_array()?
        .iter()
        .find(|os| os.pointer("/operatingSystem/name").and_then(Value::as_str) == Some(name))?
        .get("systemRequirements")?
        .as_array()?;
    let level = |wanted: &str| -> Vec<(String, String)> {
        levels
            .iter()
            .find(|level| level.get("type").and_then(Value::as_str) == Some(wanted))
            .and_then(|level| level.get("requirements")?.as_array())
            .map(|lines| {
                lines
                    .iter()
                    .filter_map(|line| {
                        let label = line.get("name")?.as_str()?;
                        let value = line.get("description")?.as_str()?;
                        Some((clean_label(label), value.trim().to_owned()))
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    rows(level("minimum"), level("recommended"))
}

fn ask_epic(agent: &ureq::Agent, app: &str, system: System) -> anyhow::Result<Option<Vec<Row>>> {
    if system != System::Windows {
        return Ok(None);
    }
    let Some(namespace) = store_tasks::epic_metadata(app).and_then(|metadata| {
        metadata
            .pointer("/metadata/namespace")?
            .as_str()
            .map(str::to_owned)
    }) else {
        return Ok(None);
    };
    let query = serde_json::json!({
        "query": "query($namespace: String!) { Catalog { catalogNs(namespace: $namespace) { mappings(pageType: \"productHome\") { pageSlug pageType } } } }",
        "variables": { "namespace": namespace },
    });
    let mut answer = agent
        .post(EPIC_GRAPHQL)
        .header("User-Agent", EPIC_AGENT)
        .header("Content-Type", "application/json")
        .send(query.to_string())
        .context("could not ask Epic for the store page")?;
    if !answer.status().is_success() {
        return Ok(None);
    }
    let found: Value = serde_json::from_str(&answer.body_mut().read_to_string()?)
        .context("Epic's store page answer is not JSON")?;
    let Some(slug) = found
        .pointer("/data/Catalog/catalogNs/mappings/0/pageSlug")
        .and_then(Value::as_str)
    else {
        return Ok(None);
    };
    let url = format!("{EPIC_CONTENT}/{slug}");
    let text = get_text(agent.get(&url)).context("could not ask Epic for the requirements")?;
    Ok(text.and_then(|text| parse_epic(&text)))
}

fn parse_epic(text: &str) -> Option<Vec<Row>> {
    let product: Value = serde_json::from_str(text).ok()?;
    let details = product
        .get("pages")?
        .as_array()?
        .iter()
        .find(|page| page.get("type").and_then(Value::as_str) == Some("productHome"))?
        .pointer("/data/requirements/systems")?
        .as_array()?
        .iter()
        .find(|system| {
            system
                .get("systemType")
                .and_then(Value::as_str)
                .is_some_and(|kind| kind.eq_ignore_ascii_case("windows"))
        })?
        .get("details")?
        .as_array()?;
    let value = |detail: &Value, level: &str| {
        detail
            .get(level)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    let found: Vec<Row> = details
        .iter()
        .filter_map(|detail| {
            let row = Row {
                label: clean_label(detail.get("title")?.as_str()?),
                minimum: value(detail, "minimum"),
                recommended: value(detail, "recommended"),
            };
            (row.minimum.is_some() || row.recommended.is_some()).then_some(row)
        })
        .collect();
    (!found.is_empty()).then_some(found)
}

fn ask_steam(agent: &ureq::Agent, app: u32, system: System) -> anyhow::Result<Option<Vec<Row>>> {
    let text = get_text(
        agent
            .get(STEAM_DETAILS)
            .query("appids", app.to_string())
            .query("l", "english"),
    )
    .context("could not ask Steam for the requirements")?;
    Ok(text.and_then(|text| parse_steam(&text, app, system)))
}

fn parse_steam(text: &str, app: u32, system: System) -> Option<Vec<Row>> {
    let reply: Value = serde_json::from_str(text).ok()?;
    let field = match system {
        System::Windows => "pc_requirements",
        System::Linux => "linux_requirements",
    };
    let wanted = reply
        .get(app.to_string())?
        .pointer(&format!("/data/{field}"))?;
    let level = |name: &str| -> Vec<(String, String)> {
        wanted
            .get(name)
            .and_then(Value::as_str)
            .and_then(owned::plain_text)
            .map(|text| steam_lines(&text))
            .unwrap_or_default()
    };
    rows(level("minimum"), level("recommended"))
}

/// Steam's lines as label and value: "Memory: 8 GB RAM", or a line with no
/// label. The heading ("Minimum:") is left out.
fn steam_lines(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| match line.split_once(':') {
            Some((label, value)) if label.len() <= 40 && !label.contains("http") => {
                let value = value.trim();
                (!value.is_empty()).then(|| (clean_label(label), value.to_owned()))
            }
            _ => Some((String::new(), line.to_owned())),
        })
        .collect()
}

fn clean_label(label: &str) -> String {
    label.trim().trim_end_matches(':').trim().to_owned()
}

/// Both levels in one list, by label, in the order the minimum gives them.
fn rows(minimum: Vec<(String, String)>, recommended: Vec<(String, String)>) -> Option<Vec<Row>> {
    let mut found: Vec<Row> = minimum
        .into_iter()
        .map(|(label, value)| Row {
            label,
            minimum: Some(value),
            recommended: None,
        })
        .collect();
    for (label, value) in recommended {
        let same = found.iter_mut().find(|row| {
            !label.is_empty() && row.recommended.is_none() && row.label.eq_ignore_ascii_case(&label)
        });
        match same {
            Some(row) => row.recommended = Some(value),
            None => found.push(Row {
                label,
                minimum: None,
                recommended: Some(value),
            }),
        }
    }
    (!found.is_empty()).then_some(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(label: &str, minimum: Option<&str>, recommended: Option<&str>) -> Row {
        Row {
            label: label.into(),
            minimum: minimum.map(Into::into),
            recommended: recommended.map(Into::into),
        }
    }

    #[test]
    fn reads_steams_lines() {
        // Cyberpunk 2077's, 2026-11-09, cut down.
        let text = r#"{"1091500": {"success": true, "data": {
            "pc_requirements": {
              "minimum": "<strong>Minimum:</strong><br><ul class=\"bb_ul\"><li>Requires a 64-bit processor and operating system<br></li><li><strong>OS:</strong> 64-bit Windows 10<br></li><li><strong>Memory:</strong> 12 GB RAM<br></li></ul>",
              "recommended": "<strong>Recommended:</strong><br><ul class=\"bb_ul\"><li>Requires a 64-bit processor and operating system<br></li><li><strong>Memory:</strong> 16 GB RAM<br></li><li><strong>Additional Notes:</strong> SSD required.</li></ul>"},
            "linux_requirements": []}}}"#;
        assert_eq!(
            parse_steam(text, 1_091_500, System::Windows).unwrap(),
            [
                row(
                    "",
                    Some("Requires a 64-bit processor and operating system"),
                    None
                ),
                row("OS", Some("64-bit Windows 10"), None),
                row("Memory", Some("12 GB RAM"), Some("16 GB RAM")),
                row(
                    "",
                    None,
                    Some("Requires a 64-bit processor and operating system")
                ),
                row("Additional Notes", None, Some("SSD required.")),
            ]
        );
        assert!(
            parse_steam(text, 1_091_500, System::Linux).is_none(),
            "[] is none"
        );
        assert!(parse_steam(text, 1, System::Windows).is_none());
    }

    #[test]
    fn reads_gogs_levels() {
        // Conarium's, 2026-11-09, cut down.
        let text = r#"{"_embedded": {"supportedOperatingSystems": [
            {"operatingSystem": {"name": "windows"}, "systemRequirements": [
              {"type": "minimum", "requirements": [
                {"id": "system", "name": "System:", "description": "Windows 7 / 8 / 10"},
                {"id": "memory", "name": "Memory:", "description": "6 GB RAM"}]},
              {"type": "recommended", "requirements": [
                {"id": "memory", "name": "Memory:", "description": "8 GB RAM"}]}]},
            {"operatingSystem": {"name": "linux"}, "systemRequirements": []}]}}"#;
        assert_eq!(
            parse_gog(text, System::Windows).unwrap(),
            [
                row("System", Some("Windows 7 / 8 / 10"), None),
                row("Memory", Some("6 GB RAM"), Some("8 GB RAM")),
            ]
        );
        assert!(parse_gog(text, System::Linux).is_none(), "none listed");
        assert!(parse_gog("{}", System::Windows).is_none());
    }

    #[test]
    fn reads_epics_details() {
        let text = r#"{"pages": [{"type": "productHome", "data": {"requirements": {"systems": [
            {"systemType": "Windows", "details": [
              {"title": "OS", "minimum": "Windows 10", "recommended": "Windows 10"},
              {"title": "Processor", "minimum": "Intel Core i7-6700", "recommended": ""},
              {"title": "Empty", "minimum": "", "recommended": ""}]}]}}}]}"#;
        assert_eq!(
            parse_epic(text).unwrap(),
            [
                row("OS", Some("Windows 10"), Some("Windows 10")),
                row("Processor", Some("Intel Core i7-6700"), None),
            ]
        );
        assert!(parse_epic(r#"{"pages": []}"#).is_none());
    }

    /// Run with `XDG_CACHE_HOME` and `XDG_DATA_HOME` in a scratch folder,
    /// the data one holding `zendo/stores/epic/metadata/Wombat.json`
    /// (World War Z's).
    #[test]
    #[ignore = "asks the real GOG, Epic and Steam"]
    fn asks_the_stores() {
        let conarium = for_game("gog:1528332522", None, System::Linux)
            .unwrap()
            .unwrap();
        assert_eq!((conarium.from, conarium.system), (From::Gog, System::Linux));
        let cyberpunk = for_game("gog:1423049311", Some(1_091_500), System::Windows)
            .unwrap()
            .unwrap();
        assert_eq!(cyberpunk.from, From::Steam, "GOG lists none for it");
        assert!(cyberpunk.has_recommended());
        let world_war_z = for_game("epic:Wombat", None, System::Windows)
            .unwrap()
            .unwrap();
        assert_eq!(world_war_z.from, From::Epic);
        assert!(for_game("gog:1", None, System::Windows).unwrap().is_none());
    }
}
