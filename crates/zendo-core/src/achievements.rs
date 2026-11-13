//! A game's achievements and which ones the player has unlocked, from the
//! game's store:
//!
//! - GOG: `gameplay.gog.com/clients/<product id>/users/<user>/achievements`
//!   with gogdl's token, as Heroic asks (the product id answers as the
//!   client id does): each with `name`, `description`, `date_unlocked`,
//!   `rarity` (percent), `visible` and both images.
//! - Steam: `IPlayerService/GetGameAchievements` (no key) for the list
//!   (`localized_name`, `localized_desc`, `icon`, `icon_gray`, `hidden`,
//!   `player_percent_unlocked`), and the player's unlocks from their
//!   profile's public stats page,
//!   `steamcommunity.com/profiles/<id>/stats/<app>/achievements/?xml=1`
//!   (`apiname`, `closed="1"`, `unlockTimestamp`), which answers only
//!   while the profile's game details are public. Its names differ in
//!   case from the list's, so they are matched ignoring it.
//! - Epic: the store GraphQL's
//!   `Achievement.productAchievementsRecordBySandbox` by the game's
//!   namespace (no sign-in), and the player's unlocks from
//!   `PlayerProfile.playerProfile(...).productAchievements` with
//!   legendary's token.
//!
//! Unlocks are asked anew each session; the icons are kept in
//! `$XDG_CACHE_HOME/zendo/achievements/`, named after their address.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use serde::Deserialize;
use serde_json::Value;

use crate::stores::Store;
use crate::{paths, store_tasks, time};

const GOG_GAMEPLAY: &str = "https://gameplay.gog.com/clients";
const STEAM_LIST: &str = "https://api.steampowered.com/IPlayerService/GetGameAchievements/v1/";
/// Where Steam's profile pages take the icons from; the older
/// `cdn.fastly.steamstatic.com/steamcommunity/public/images/apps` lacks
/// some (Rust's grey ones, 2026-11-09).
const STEAM_ICONS: &str = "https://shared.fastly.steamstatic.com/community_assets/images/apps";
const EPIC_GRAPHQL: &str = "https://store.epicgames.com/graphql";
/// Epic's GraphQL answers the launcher, as Heroic sends it.
const EPIC_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) EpicGamesLauncher";
/// How many icons are downloaded at once.
const ICON_WORKERS: usize = 8;

/// One achievement.
#[derive(Debug, Clone, PartialEq)]
pub struct Achievement {
    pub name: String,
    pub description: String,
    /// Kept back until unlocked.
    pub hidden: bool,
    /// When the player unlocked it, in Unix seconds.
    pub unlocked: Option<u64>,
    /// The share of players who have it, in percent.
    pub rarity: Option<f32>,
    /// Its icon as it stands (locked or not), once downloaded; none for a
    /// hidden one still locked.
    pub icon: Option<PathBuf>,
}

/// A game's achievements.
#[derive(Debug, Clone, PartialEq)]
pub struct Achievements {
    /// Unlocked first, newest first; then locked, most common first;
    /// hidden ones still locked last.
    pub list: Vec<Achievement>,
    /// Whether the store said which ones the player has; not for a Steam
    /// profile whose game details are private, or a sign-in gone stale.
    pub unlocks_known: bool,
}

impl Achievements {
    /// How many the player has unlocked.
    #[must_use]
    pub fn unlocked(&self) -> usize {
        self.list
            .iter()
            .filter(|achievement| achievement.unlocked.is_some())
            .count()
    }
}

/// One as a store gives it, before its icon is downloaded.
struct Found {
    /// The store's own name for it, to match unlocks by.
    key: String,
    name: String,
    description: String,
    hidden: bool,
    unlocked: Option<u64>,
    rarity: Option<f32>,
    icon_unlocked: Option<String>,
    icon_locked: Option<String>,
}

/// The game's achievements, by its Zendo id; `steam_id` is the player's
/// 64-bit Steam id, for a Steam game's unlocks. `None` when the game's
/// store gives none, or Zendo cannot ask it (a local game, GOG signed
/// out). It blocks on the network; the window calls it on a thread.
///
/// # Errors
///
/// When a store cannot be reached or answers oddly.
pub fn for_game(id: &str, steam_id: Option<&str>) -> anyhow::Result<Option<Achievements>> {
    let agent = crate::http::answering_agent(Duration::from_secs(20));
    let (store, app) = id.split_once(':').unwrap_or(("", id));
    let found = match store {
        "gog" => ask_gog(&agent, app)?,
        "steam" => match app.parse() {
            Ok(app) => ask_steam(&agent, app, steam_id)?,
            Err(_) => None,
        },
        "epic" => ask_epic(&agent, app)?,
        _ => None,
    };
    let Some((found, unlocks_known)) = found.filter(|(found, _)| !found.is_empty()) else {
        return Ok(None);
    };
    let mut list = with_icons(found);
    sort(&mut list);
    Ok(Some(Achievements {
        list,
        unlocks_known,
    }))
}

fn sort(list: &mut [Achievement]) {
    list.sort_by(|a, b| {
        let rank = |item: &Achievement| match item.unlocked {
            Some(_) => 0,
            None if item.hidden => 2,
            None => 1,
        };
        rank(a)
            .cmp(&rank(b))
            .then_with(|| b.unlocked.cmp(&a.unlocked))
            .then_with(|| b.rarity.unwrap_or(0.0).total_cmp(&a.rarity.unwrap_or(0.0)))
    });
}

fn call_text(
    request: ureq::RequestBuilder<ureq::typestate::WithoutBody>,
) -> anyhow::Result<Option<String>> {
    let mut answer = request.call()?;
    let status = answer.status();
    if status.is_success() {
        return Ok(Some(
            answer
                .body_mut()
                .read_to_string()
                .context("the store's achievements answer")?,
        ));
    }
    // Steam answers 403 for an app with no achievements, and a store that
    // does not know the game answers 404: both mean "none". The rest are
    // news, and hiding them would say "no achievements" for an outage.
    anyhow::ensure!(
        matches!(status.as_u16(), 403 | 404),
        "{status} from the store"
    );
    Ok(None)
}

fn ask_gog(agent: &ureq::Agent, product: &str) -> anyhow::Result<Option<(Vec<Found>, bool)>> {
    let Some(credentials) = Store::Gog.gog_token()? else {
        return Ok(None);
    };
    let url = format!(
        "{GOG_GAMEPLAY}/{product}/users/{}/achievements",
        credentials.user_id
    );
    let text = call_text(
        agent
            .get(&url)
            .header(
                "Authorization",
                format!("Bearer {}", credentials.access_token),
            )
            .header("X-Gog-Lc", "en-US"),
    )
    .context("could not ask GOG for the achievements")?;
    Ok(text
        .map(|text| parse_gog(&text))
        .transpose()?
        .map(|found| (found, true)))
}

fn parse_gog(text: &str) -> anyhow::Result<Vec<Found>> {
    #[derive(Deserialize)]
    struct Answer {
        #[serde(default)]
        items: Vec<Item>,
    }
    #[derive(Deserialize)]
    struct Item {
        achievement_key: String,
        name: String,
        #[serde(default)]
        description: String,
        date_unlocked: Option<String>,
        rarity: Option<f32>,
        #[serde(default = "shown")]
        visible: bool,
        image_url_unlocked: Option<String>,
        image_url_locked: Option<String>,
    }
    fn shown() -> bool {
        true
    }
    let answer: Answer = serde_json::from_str(text).context("GOG's achievements are not valid")?;
    Ok(answer
        .items
        .into_iter()
        .map(|item| Found {
            key: item.achievement_key,
            name: item.name,
            description: item.description,
            hidden: !item.visible,
            unlocked: item.date_unlocked.as_deref().and_then(parse_date),
            rarity: item.rarity,
            icon_unlocked: item.image_url_unlocked,
            icon_locked: item.image_url_locked,
        })
        .collect())
}

/// GOG's `2026-09-25T15:49:09+0000`, or Epic's `…Z`, as Unix seconds.
fn parse_date(text: &str) -> Option<u64> {
    let utc = text
        .strip_suffix("+0000")
        .or_else(|| text.strip_suffix("+00:00"))
        .map_or_else(|| text.to_owned(), |base| format!("{base}Z"));
    time::parse_utc(&utc)
}

fn ask_steam(
    agent: &ureq::Agent,
    app: u32,
    steam_id: Option<&str>,
) -> anyhow::Result<Option<(Vec<Found>, bool)>> {
    let text = call_text(
        agent
            .get(STEAM_LIST)
            .query("appid", app.to_string())
            .query("language", "english"),
    )
    .context("could not ask Steam for the achievements")?;
    let Some(mut found) = text.map(|text| parse_steam_list(&text, app)).transpose()? else {
        return Ok(None);
    };
    let unlocks = match steam_id {
        Some(steam_id) => {
            let url =
                format!("https://steamcommunity.com/profiles/{steam_id}/stats/{app}/achievements/");
            call_text(agent.get(&url).query("xml", "1"))
                .context("could not ask Steam for the unlocks")?
                .and_then(|xml| steam_unlocks(&xml))
        }
        None => None,
    };
    let known = unlocks.is_some();
    if let Some(unlocks) = unlocks {
        for item in &mut found {
            item.unlocked = unlocks.get(&item.key.to_lowercase()).copied();
        }
    }
    Ok(Some((found, known)))
}

fn parse_steam_list(text: &str, app: u32) -> anyhow::Result<Vec<Found>> {
    #[derive(Deserialize)]
    struct Answer {
        response: Response,
    }
    #[derive(Deserialize)]
    struct Response {
        #[serde(default)]
        achievements: Vec<Item>,
    }
    #[derive(Deserialize)]
    struct Item {
        internal_name: String,
        localized_name: String,
        #[serde(default)]
        localized_desc: String,
        icon: Option<String>,
        icon_gray: Option<String>,
        #[serde(default)]
        hidden: bool,
        player_percent_unlocked: Option<String>,
    }
    let answer: Answer =
        serde_json::from_str(text).context("Steam's achievements are not valid")?;
    let icon = |file: Option<String>| {
        file.filter(|file| !file.is_empty())
            .map(|file| format!("{STEAM_ICONS}/{app}/{file}"))
    };
    Ok(answer
        .response
        .achievements
        .into_iter()
        .map(|item| Found {
            key: item.internal_name,
            name: item.localized_name,
            description: item.localized_desc,
            hidden: item.hidden,
            unlocked: None,
            rarity: item
                .player_percent_unlocked
                .and_then(|percent| percent.parse().ok()),
            icon_unlocked: icon(item.icon),
            icon_locked: icon(item.icon_gray),
        })
        .collect())
}

/// The profile page's unlocks, by `apiname` in lower case: `None` when the
/// page gives no stats (a private profile answers with an error).
fn steam_unlocks(xml: &str) -> Option<HashMap<String, u64>> {
    if !xml.contains("<playerstats>") || !xml.contains("<achievements>") {
        return None;
    }
    let mut unlocks = HashMap::new();
    for block in xml.split("<achievement ").skip(1) {
        // The attributes' order carries no meaning: `closed` can come
        // anywhere in the open tag.
        let tag = block.split_once('>').map_or(block, |(tag, _)| tag);
        if !tag
            .split_whitespace()
            .any(|attribute| attribute == "closed=\"1\"")
        {
            continue;
        }
        let field = |name: &str| {
            let open = format!("<{name}>");
            let close = format!("</{name}>");
            let start = block.find(&open)? + open.len();
            let end = start + block[start..].find(&close)?;
            let value = block[start..end].trim();
            Some(
                value
                    .strip_prefix("<![CDATA[")
                    .and_then(|value| value.strip_suffix("]]>"))
                    .unwrap_or(value)
                    .to_owned(),
            )
        };
        if let Some(name) = field("apiname") {
            let when = field("unlockTimestamp")
                .and_then(|when| when.parse().ok())
                .unwrap_or(0);
            unlocks.insert(name.to_lowercase(), when);
        }
    }
    Some(unlocks)
}

fn ask_epic(agent: &ureq::Agent, app: &str) -> anyhow::Result<Option<(Vec<Found>, bool)>> {
    let Some(namespace) = store_tasks::epic_metadata(app).and_then(|metadata| {
        metadata
            .pointer("/metadata/namespace")?
            .as_str()
            .map(str::to_owned)
    }) else {
        return Ok(None);
    };
    let list = epic_query(
        agent,
        None,
        "query($sandbox: String!) { Achievement { productAchievementsRecordBySandbox(sandboxId: $sandbox, locale: \"en-US\") { productId achievements { achievement { name hidden unlockedDisplayName unlockedDescription unlockedIconLink lockedIconLink rarity { percent } } } } } }",
        &serde_json::json!({ "sandbox": namespace }),
    )?;
    let Some(record) = list
        .as_ref()
        .and_then(|list| list.pointer("/data/Achievement/productAchievementsRecordBySandbox"))
        .filter(|record| !record.is_null())
    else {
        return Ok(None);
    };
    let mut found = parse_epic_list(record);
    let product = record.get("productId").and_then(Value::as_str);
    let unlocks = match (product, epic_user()) {
        (Some(product), Some((account, token))) => epic_query(
            agent,
            Some(&token),
            "query($account: String!, $product: String!) { PlayerProfile { playerProfile(epicAccountId: $account) { productAchievements(productId: $product) { ... on PlayerProductAchievementsResponseSuccess { data { playerAchievements { playerAchievement { achievementName unlocked unlockDate } } } } } } } }",
            &serde_json::json!({ "account": account, "product": product }),
        )?
        .and_then(|answer| epic_unlocks(&answer)),
        _ => None,
    };
    let known = unlocks.is_some();
    if let Some(unlocks) = unlocks {
        for item in &mut found {
            item.unlocked = unlocks.get(&item.key).copied();
        }
    }
    Ok(Some((found, known)))
}

/// legendary's account id and token, from Zendo's Epic folder.
fn epic_user() -> Option<(String, String)> {
    let file = Store::Epic.dir()?.join("user.json");
    let user: Value = serde_json::from_slice(&fs::read(file).ok()?).ok()?;
    Some((
        user.get("account_id")?.as_str()?.to_owned(),
        user.get("access_token")?.as_str()?.to_owned(),
    ))
}

fn epic_query(
    agent: &ureq::Agent,
    token: Option<&str>,
    query: &str,
    variables: &Value,
) -> anyhow::Result<Option<Value>> {
    let body = serde_json::json!({ "query": query, "variables": variables });
    let mut request = agent
        .post(EPIC_GRAPHQL)
        .header("User-Agent", EPIC_AGENT)
        .header("Content-Type", "application/json");
    if let Some(token) = token {
        request = request.header("Authorization", format!("bearer {token}"));
    }
    let mut answer = request
        .send(body.to_string())
        .context("could not ask Epic for the achievements")?;
    if !answer.status().is_success() {
        return Ok(None);
    }
    let text = answer
        .body_mut()
        .read_to_string()
        .context("Epic's achievements answer")?;
    Ok(serde_json::from_str(&text).ok())
}

fn parse_epic_list(record: &Value) -> Vec<Found> {
    let text = |item: &Value, field: &str| {
        item.get(field)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    record
        .get("achievements")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let item = entry.get("achievement")?;
            let link = |field: &str| Some(text(item, field)).filter(|link| !link.is_empty());
            Some(Found {
                key: item.get("name")?.as_str()?.to_owned(),
                name: text(item, "unlockedDisplayName"),
                description: text(item, "unlockedDescription"),
                hidden: item.get("hidden").and_then(Value::as_bool).unwrap_or(false),
                unlocked: None,
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "a percentage, well within f32"
                )]
                rarity: item
                    .pointer("/rarity/percent")
                    .and_then(Value::as_f64)
                    .map(|percent| percent as f32),
                icon_unlocked: link("unlockedIconLink"),
                icon_locked: link("lockedIconLink"),
            })
        })
        .collect()
}

/// The player's unlocks by achievement name: `None` when Epic refused (a
/// stale token) or answered no data.
fn epic_unlocks(answer: &Value) -> Option<HashMap<String, u64>> {
    let list = answer
        .pointer("/data/PlayerProfile/playerProfile/productAchievements/data/playerAchievements")?
        .as_array()?;
    Some(
        list.iter()
            .filter_map(|entry| {
                let item = entry.get("playerAchievement")?;
                if item.get("unlocked").and_then(Value::as_bool) != Some(true) {
                    return None;
                }
                let when = item
                    .get("unlockDate")
                    .and_then(Value::as_str)
                    .and_then(parse_date)
                    .unwrap_or(0);
                Some((item.get("achievementName")?.as_str()?.to_owned(), when))
            })
            .collect(),
    )
}

/// The achievements with their icons downloaded, a few at once and each
/// address once (locked ones often share an icon); an icon that cannot be
/// downloaded is left out.
fn with_icons(found: Vec<Found>) -> Vec<Achievement> {
    let wanted: Vec<Option<String>> = found
        .iter()
        .map(|item| match (item.unlocked, item.hidden) {
            (Some(_), _) => item.icon_unlocked.clone(),
            (None, false) => item
                .icon_locked
                .clone()
                .or_else(|| item.icon_unlocked.clone()),
            (None, true) => None,
        })
        .collect();
    let mut urls: Vec<&str> = wanted.iter().flatten().map(String::as_str).collect();
    urls.sort_unstable();
    urls.dedup();
    let mut files: Vec<Option<PathBuf>> = vec![None; urls.len()];
    let dir = paths::cache_home().map(|dir| dir.join("zendo/achievements"));
    if let Some(dir) = dir.filter(|dir| fs::create_dir_all(dir).is_ok()) {
        let chunk = urls.len().div_ceil(ICON_WORKERS).max(1);
        std::thread::scope(|scope| {
            for (urls, files) in urls.chunks(chunk).zip(files.chunks_mut(chunk)) {
                let dir = &dir;
                scope.spawn(move || {
                    for (url, file) in urls.iter().zip(files) {
                        *file = crate::http::cached_image(dir, url);
                    }
                });
            }
        });
    }
    let downloaded: HashMap<&str, PathBuf> = urls
        .into_iter()
        .zip(files)
        .filter_map(|(url, file)| Some((url, file?)))
        .collect();
    let icons: Vec<Option<PathBuf>> = wanted
        .iter()
        .map(|url| downloaded.get(url.as_deref()?).cloned())
        .collect();
    found
        .into_iter()
        .zip(icons)
        .map(|(item, icon)| Achievement {
            name: item.name,
            description: item.description,
            hidden: item.hidden,
            unlocked: item.unlocked,
            rarity: item.rarity,
            icon,
        })
        .collect()
}


#[cfg(test)]
mod tests {
    use super::*;

    fn found(key: &str) -> Found {
        Found {
            key: key.into(),
            name: key.into(),
            description: String::new(),
            hidden: false,
            unlocked: None,
            rarity: None,
            icon_unlocked: None,
            icon_locked: None,
        }
    }

    #[test]
    fn reads_gogs_list() {
        // Cyberpunk 2077's, 2026-11-09, cut down.
        let text = r#"{"total_count": 2, "items": [
            {"achievement_id": "1", "achievement_key": "AllThePresidentsMen", "visible": true,
             "name": "All the President's Men", "description": "Complete the job.",
             "image_url_unlocked": "https://images.gog.com/u.png", "image_url_locked": "https://images.gog.com/l.png",
             "date_unlocked": null, "rarity": 25.4, "rarity_level_description": "Uncommon", "rarity_level_slug": "uncommon"},
            {"achievement_id": "2", "achievement_key": "Secret", "visible": false, "name": "Secret",
             "description": "", "date_unlocked": "2026-09-25T15:49:09+0000", "rarity": 3.1}]}"#;
        let list = parse_gog(text).unwrap();
        assert_eq!(list[0].key, "AllThePresidentsMen");
        assert_eq!(list[0].rarity, Some(25.4));
        assert!(list[0].unlocked.is_none());
        assert!(list[1].hidden);
        assert_eq!(list[1].unlocked, Some(1_790_351_349));
    }

    #[test]
    fn matches_steams_unlocks_ignoring_case() {
        let list = r#"{"response": {"achievements": [
            {"internal_name": "PLACE_CAMPFIRE", "localized_name": "Place Camp Fire",
             "localized_desc": "Place a camp fire", "icon": "a.jpg", "icon_gray": "b.jpg",
             "hidden": false, "player_percent_unlocked": "34.8"}]}}"#;
        let found = parse_steam_list(list, 252_490).unwrap();
        assert_eq!(
            found[0].icon_locked.as_deref(),
            Some("https://shared.fastly.steamstatic.com/community_assets/images/apps/252490/b.jpg")
        );
        assert_eq!(found[0].rarity, Some(34.8));
        let xml = r#"<?xml version="1.0"?><playerstats><achievements>
            <achievement closed="1"><name><![CDATA[Place Camp Fire]]></name>
              <apiname><![CDATA[place_campfire]]></apiname><unlockTimestamp>1659102640</unlockTimestamp></achievement>
            <achievement closed="0"><apiname><![CDATA[craft_campfire]]></apiname></achievement>
            </achievements></playerstats>"#;
        let unlocks = steam_unlocks(xml).unwrap();
        assert_eq!(unlocks.get("place_campfire"), Some(&1_659_102_640));
        assert_eq!(unlocks.len(), 1);
        // An attribute before `closed` does not hide the unlock.
        let reordered = xml.replace(
            r#"<achievement closed="1">"#,
            r#"<achievement name="Place Camp Fire" closed="1">"#,
        );
        let unlocks = steam_unlocks(&reordered).unwrap();
        assert_eq!(unlocks.get("place_campfire"), Some(&1_659_102_640));
        let private = "<?xml version=\"1.0\"?><response><error><![CDATA[This profile is private.]]></error></response>";
        assert!(steam_unlocks(private).is_none());
    }

    #[test]
    fn reads_epics_list_and_unlocks() {
        let record = serde_json::json!({"productId": "p", "achievements": [
            {"achievement": {"name": "10", "hidden": false, "unlockedDisplayName": "Virtuoso",
             "unlockedDescription": "Unlock all", "unlockedIconLink": "https://x/u", "lockedIconLink": "https://x/l",
             "rarity": {"percent": 0.1}}}]});
        let found = parse_epic_list(&record);
        assert_eq!(
            (found[0].key.as_str(), found[0].name.as_str()),
            ("10", "Virtuoso")
        );
        let answer = serde_json::json!({"data": {"PlayerProfile": {"playerProfile": {"productAchievements": {"data": {
            "playerAchievements": [
              {"playerAchievement": {"achievementName": "10", "unlocked": true, "unlockDate": "2026-02-09T18:19:14.776Z"}},
              {"playerAchievement": {"achievementName": "11", "unlocked": false, "unlockDate": null}}]}}}}}});
        let unlocks = epic_unlocks(&answer).unwrap();
        assert_eq!(unlocks.get("10"), Some(&1_770_661_154));
        assert!(!unlocks.contains_key("11"));
        let refused = serde_json::json!({"errors": [{"message": "403"}], "data": {"PlayerProfile": {"playerProfile": null}}});
        assert!(epic_unlocks(&refused).is_none());
    }

    #[test]
    fn sorts_unlocked_newest_then_common_then_hidden() {
        let mut list: Vec<Achievement> = [
            ("rare", None, false, 2.0),
            ("hidden", None, true, 50.0),
            ("old", Some(10), false, 1.0),
            ("common", None, false, 80.0),
            ("new", Some(20), false, 1.0),
        ]
        .into_iter()
        .map(|(name, unlocked, hidden, rarity)| Achievement {
            unlocked,
            hidden,
            rarity: Some(rarity),
            ..with_icons(vec![found(name)]).remove(0)
        })
        .collect();
        sort(&mut list);
        let names: Vec<&str> = list.iter().map(|item| item.name.as_str()).collect();
        assert_eq!(names, ["new", "old", "common", "rare", "hidden"]);
        let achievements = Achievements {
            list,
            unlocks_known: true,
        };
        assert_eq!(achievements.unlocked(), 2);
    }

    /// Run with `XDG_CACHE_HOME` in a scratch folder. Asks Steam only,
    /// without a profile, so no sign-in is used.
    #[test]
    #[ignore = "asks the real Steam"]
    fn asks_steam() {
        let rust = for_game("steam:252490", None).unwrap().unwrap();
        assert_eq!(rust.list.len(), 116);
        assert!(!rust.unlocks_known);
        assert!(rust.list.iter().any(|item| item.icon.is_some()));
    }
}
