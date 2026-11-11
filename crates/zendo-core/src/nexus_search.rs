//! Searching a game's mods on Nexus Mods, through its GraphQL API (v2),
//! which answers searches without a key. Downloading stays with
//! [`crate::nexus`]: its v1 API, the user's key, and the site's Mod manager
//! download for accounts without Premium.

use std::time::Duration;

use anyhow::{Context, bail};
use serde::Deserialize;
use serde_json::json;

const GRAPHQL: &str = "https://api.nexusmods.com/v2/graphql";

/// How many mods a search brings at a time.
pub const PAGE: u32 = 20;

const QUERY: &str =
    "query Search($filter: ModsFilter, $sort: [ModsSort!], $count: Int, $offset: Int) {
  mods(filter: $filter, sort: $sort, count: $count, offset: $offset) {
    totalCount
    nodes {
      modId name summary author version endorsements downloads fileSize
      updatedAt adultContent thumbnailUrl modCategory { name }
    }
  }
}";

/// The order of the results. Nexus's own `relevance` is left out: tried on
/// a few games, it put little-known mods before the one searched for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Order {
    #[default]
    Endorsed,
    Downloaded,
    Updated,
}

impl Order {
    pub const ALL: [Self; 3] = [Self::Endorsed, Self::Downloaded, Self::Updated];

    fn field(self) -> &'static str {
        match self {
            Self::Endorsed => "endorsements",
            Self::Downloaded => "downloads",
            Self::Updated => "updatedAt",
        }
    }
}

/// A search of one game's mods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    /// The game's name on Nexus (`stardewvalley`).
    pub domain: String,
    /// Words the mod's name holds; none lists every mod.
    pub words: String,
    pub order: Order,
    /// Whether mods marked adult are shown.
    pub adult: bool,
    /// How many results to skip: those already shown.
    pub offset: u32,
}

/// One mod found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub mod_id: u64,
    pub name: String,
    pub summary: String,
    pub author: String,
    pub version: String,
    pub endorsements: u64,
    pub downloads: u64,
    /// In bytes, when Nexus says.
    pub size: Option<u64>,
    /// When it last changed, `YYYY-MM-DD`.
    pub updated: String,
    pub category: Option<String>,
    pub adult: bool,
    pub thumbnail: Option<String>,
}

/// A page of results, and how many there are in all.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    pub hits: Vec<Hit>,
    pub total: u64,
}

/// Searches a game's mods.
///
/// # Errors
///
/// When Nexus cannot be reached or answers with an error.
pub fn search(query: &Query) -> anyhow::Result<Found> {
    let response = crate::http::answering_agent(Duration::from_secs(30))
        .post(GRAPHQL)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("Application-Name", "Zendo")
        .header("Application-Version", env!("CARGO_PKG_VERSION"))
        .send(request(query).to_string())
        .context("could not reach Nexus Mods")?;
    let status = response.status().as_u16();
    let text = response
        .into_body()
        .read_to_string()
        .context("Nexus's search")?;
    match status {
        200..=299 => parse(&text),
        429 => bail!("Nexus's limit of calls for now was reached; try again in a while"),
        code => bail!("Nexus's search answered {code}"),
    }
}

/// The request's body: the query and its variables.
fn request(query: &Query) -> serde_json::Value {
    let mut filter = json!({
        "gameDomainName": [{"value": query.domain, "op": "EQUALS"}],
    });
    let words = query.words.split_whitespace().collect::<Vec<_>>().join(" ");
    if !words.is_empty() {
        filter["name"] = json!([{"value": words, "op": "WILDCARD"}]);
    }
    if !query.adult {
        filter["adultContent"] = json!([{"value": false, "op": "EQUALS"}]);
    }
    json!({
        "query": QUERY,
        "variables": {
            "filter": filter,
            "sort": [{query.order.field(): {"direction": "DESC"}}],
            "count": PAGE,
            "offset": query.offset,
        },
    })
}

fn parse(text: &str) -> anyhow::Result<Found> {
    #[derive(Deserialize)]
    struct Reply {
        data: Option<Data>,
        #[serde(default)]
        errors: Vec<Error>,
    }
    #[derive(Deserialize)]
    struct Error {
        message: String,
    }
    #[derive(Deserialize)]
    struct Data {
        mods: Mods,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Mods {
        #[serde(default)]
        total_count: u64,
        #[serde(default)]
        nodes: Vec<Node>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Node {
        mod_id: u64,
        name: String,
        #[serde(default)]
        summary: Option<String>,
        #[serde(default)]
        author: Option<String>,
        #[serde(default)]
        version: Option<String>,
        #[serde(default)]
        endorsements: u64,
        #[serde(default)]
        downloads: u64,
        /// In KB.
        #[serde(default)]
        file_size: Option<u64>,
        #[serde(default)]
        updated_at: String,
        #[serde(default)]
        adult_content: Option<bool>,
        #[serde(default)]
        thumbnail_url: Option<String>,
        #[serde(default)]
        mod_category: Option<Category>,
    }
    #[derive(Deserialize)]
    struct Category {
        name: String,
    }

    let reply: Reply = serde_json::from_str(text).context("Nexus's search: not JSON")?;
    if let Some(error) = reply.errors.first() {
        bail!("Nexus's search: {}", error.message);
    }
    let mods = reply.data.context("Nexus's search: no answer")?.mods;
    let tidy = |text: Option<String>| text.map(|text| text.trim().to_owned()).unwrap_or_default();
    Ok(Found {
        total: mods.total_count,
        hits: mods
            .nodes
            .into_iter()
            .map(|node| Hit {
                mod_id: node.mod_id,
                name: node.name.trim().to_owned(),
                summary: plain(&tidy(node.summary)),
                author: tidy(node.author),
                version: tidy(node.version),
                endorsements: node.endorsements,
                downloads: node.downloads,
                size: node
                    .file_size
                    .filter(|kb| *kb > 0)
                    .map(|kb| kb.saturating_mul(1024)),
                updated: node.updated_at.chars().take(10).collect(),
                category: node.mod_category.map(|category| category.name),
                adult: node.adult_content.unwrap_or(false),
                thumbnail: node.thumbnail_url.filter(|url| url.starts_with("https://")),
            })
            .collect(),
    })
}

/// A summary as plain text: Nexus's carry `<br />` and HTML entities.
fn plain(summary: &str) -> String {
    let text = summary
        .replace("<br />", " ")
        .replace("<br/>", " ")
        .replace("<br>", " ")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(words: &str, adult: bool) -> Query {
        Query {
            domain: "stardewvalley".into(),
            words: words.into(),
            order: Order::Endorsed,
            adult,
            offset: 20,
        }
    }

    #[test]
    fn asks_for_a_games_mods_by_name() {
        let body = request(&query("  content   patcher ", false));
        let variables = &body["variables"];
        assert_eq!(
            variables["filter"]["gameDomainName"][0]["value"],
            "stardewvalley"
        );
        assert_eq!(variables["filter"]["name"][0]["value"], "content patcher");
        assert_eq!(variables["filter"]["name"][0]["op"], "WILDCARD");
        assert_eq!(variables["filter"]["adultContent"][0]["value"], false);
        assert_eq!(variables["sort"][0]["endorsements"]["direction"], "DESC");
        assert_eq!(variables["offset"], 20);
        // No words: every mod; adult ones too when asked.
        let body = request(&query(" ", true));
        assert!(body["variables"]["filter"].get("name").is_none());
        assert!(body["variables"]["filter"].get("adultContent").is_none());
    }

    #[test]
    fn reads_the_mods_found() {
        let text = r#"{"data":{"mods":{"totalCount":407,"nodes":[
            {"modId":1915,"name":"Content Patcher","summary":"Loads content packs&#39; changes<br />without replacing files.",
             "author":"Pathoschild","version":"2.5.3","endorsements":496355,"downloads":9000000,
             "fileSize":4024,"updatedAt":"2025-03-01T10:00:00Z","adultContent":false,
             "thumbnailUrl":"https://staticdelivery.nexusmods.com/mods/1303/images/thumbnails/1915/1.png",
             "modCategory":{"name":"Modding Tools"}},
            {"modId":7,"name":" Old ","summary":null,"author":null,"version":null,"endorsements":0,
             "downloads":0,"fileSize":0,"updatedAt":"","adultContent":null,"thumbnailUrl":"http://x",
             "modCategory":null}]}}}"#;
        let found = parse(text).unwrap();
        assert_eq!(found.total, 407);
        let hit = &found.hits[0];
        assert_eq!(hit.mod_id, 1915);
        assert_eq!(
            hit.summary,
            "Loads content packs' changes without replacing files."
        );
        assert_eq!(hit.size, Some(4024 * 1024));
        assert_eq!(hit.updated, "2025-03-01");
        assert_eq!(hit.category.as_deref(), Some("Modding Tools"));
        let old = &found.hits[1];
        assert_eq!(old.name, "Old");
        assert_eq!((old.size, old.thumbnail.as_deref()), (None, None));
        let error = parse(r#"{"errors":[{"message":"bad filter"}],"data":null}"#).unwrap_err();
        assert_eq!(error.to_string(), "Nexus's search: bad filter");
    }

    /// Asks the real Nexus: `cargo test -p zendo-core nexus_search -- --ignored`.
    #[test]
    #[ignore = "asks the real Nexus Mods"]
    fn searches_the_real_nexus() {
        let found = search(&Query {
            offset: 0,
            ..query("smapi", false)
        })
        .unwrap();
        assert_eq!(found.hits[0].mod_id, 2400, "{:?}", found.hits[0]);
        assert!(found.total > 10);
    }
}
