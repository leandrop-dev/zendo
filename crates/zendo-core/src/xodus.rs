//! Nexus Launcher's games: PC Game Pass titles the Microsoft Store installed,
//! and the titles the account owns (`owned`, for the Store page).
//!
//! The launcher (`domsvoboda/nexus-launcher`, MIT, formerly `xodus-nexus`)
//! keeps a profile in `$XDG_CONFIG_HOME/xodus-nexus` with
//! `launcher-state.json`. A record there is a game the Microsoft Store
//! installed; it is started through the launcher's own
//! `shortcuts/launch-game <id>` script. Read only, as Steam and Heroic are
//! read: Zendo lists these games and starts them, and never installs,
//! updates or uninstalls them (`docs/reference.md`, "Nexus Launcher").

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use anyhow::Context as _;
use serde::Deserialize;

use crate::owned::{Owned, Shop};
use crate::{Details, Game, Launch, Scan, Source, engines, paths};

/// Where the launcher keeps its profile, when it is installed.
#[must_use]
pub fn profile() -> Option<PathBuf> {
    let dir = paths::config_home()?.join("xodus-nexus");
    dir.is_dir().then_some(dir)
}

/// The marker the launcher writes on line 2 of the scripts it makes. A
/// `launch-game` without it is some other file, and is never started.
const MARKER: &str = "# X-Xodus-Nexus-Profile=";

/// `domsvoboda/nexus-launcher`, whose releases carry the Linux `AppImage`.
const REPO: &str = "domsvoboda/nexus-launcher";

/// What the launcher is called in Zendo's words.
const LABEL: &str = "Nexus Launcher";

/// What the file in Zendo's launchers folder is called.
const APPIMAGE: &str = "Nexus.AppImage";

/// The `AppImage` Zendo downloaded, once it is in place.
#[must_use]
pub fn appimage() -> Option<PathBuf> {
    engines::launchers_dir().map(|dir| dir.join(APPIMAGE))
}

/// The `AppImage`, when it is there.
#[must_use]
pub fn installed() -> Option<PathBuf> {
    appimage().filter(|path| path.is_file())
}

/// How many games the launcher's profile lists as ready.
#[must_use]
pub fn count() -> usize {
    profile().map_or(0, |profile| scan(Some(&profile)).games.len())
}

/// The `AppImage` of the latest release, for this machine.
///
/// # Errors
///
/// When GitHub cannot be reached, the release has no `AppImage` for this
/// machine, or it has no checksum to check the download against.
pub fn release() -> anyhow::Result<engines::Release> {
    let (tag, asset) = engines::latest_asset(REPO, LABEL, |name| {
        appimage_for(name, std::env::consts::ARCH)
    })?;
    Ok(engines::Release {
        tag,
        asset,
        service: None,
    })
}

/// The Store's catalogue of product information.
const CATALOGUE: &str = "https://displaycatalog.mp.microsoft.com/v7.0/products";

/// How many product ids one catalogue query asks for.
const BATCH: usize = 20;

/// The market the Store answers for, as `resolve-package` asks by default.
const MARKET: &str = "US";

/// Runs the launcher's backend and returns its answer, from the first `{`:
/// its diagnostics come before it, one line each.
fn backend(args: &[&str]) -> anyhow::Result<String> {
    let cli = cli().context("Nexus Launcher's backend is not installed")?;
    let output = std::process::Command::new(&cli)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .context("could not run Nexus Launcher's backend")?;
    if !output.status.success() {
        anyhow::bail!(
            "Nexus Launcher's backend could not {}: {}",
            args.first().copied().unwrap_or("answer"),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let start = text
        .find('{')
        .context("Nexus Launcher's backend gave no answer")?;
    Ok(text[start..].to_owned())
}

/// The launcher's backend program, at the path its `current.json` names.
fn cli() -> Option<PathBuf> {
    #[derive(Deserialize)]
    struct Current {
        #[serde(rename = "binaryPath")]
        binary: PathBuf,
    }
    let file = profile()?.join("xodus-backends/current.json");
    let bytes = fs::read(file).ok()?;
    let current: Current = serde_json::from_slice(&bytes).ok()?;
    current.binary.is_file().then_some(current.binary)
}

/// Whether an `AppImage`'s name is the one this machine runs: the release
/// names them `Xodus-Nexus-<version>-x86_64.AppImage` and `-arm64`, and the
/// checksum file beside them is not an `AppImage`.
fn appimage_for(name: &str, arch: &str) -> bool {
    name.ends_with(".AppImage")
        && match arch {
            "x86_64" => name.contains("x86_64"),
            "aarch64" => name.contains("arm64") || name.contains("aarch64"),
            _ => false,
        }
}

/// Downloads the `AppImage`, checks it against its digest and puts it in
/// place, executable, reporting the bytes as it goes; `stop` set ends it.
///
/// # Errors
///
/// When the download fails or is stopped, the checksum does not match, or
/// the file cannot be put in place; the `AppImage` there before stays then.
pub fn install(
    release: &engines::Release,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> anyhow::Result<PathBuf> {
    engines::bring(release, APPIMAGE, stop, progress)
}

/// Opens the launcher in its own window: it writes its profile and shortcuts
/// the first time, and there the user installs its runtime and signs in.
/// The caller waits for it, so the profile can be read once it ends.
///
/// # Errors
///
/// When the `AppImage` is not there or cannot be started.
pub fn open() -> anyhow::Result<std::process::Child> {
    let appimage = installed().context("Nexus Launcher is not installed yet")?;
    crate::launch::spawn_logged(std::process::Command::new(appimage), "nexus")
}

/// The games the launcher's profile lists, or one problem.
pub(crate) fn scan(profile: Option<&Path>) -> Scan {
    let mut scan = Scan::default();
    let Some(profile) = profile else {
        return scan;
    };
    let file = profile.join("launcher-state.json");
    let path = file.display().to_string();
    let state: State = match fs::read(&file)
        .map_err(|err| err.to_string())
        .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|err| err.to_string()))
    {
        Ok(state) => state,
        Err(err) => {
            scan.problems.push(format!("{path}: {err}"));
            return scan;
        }
    };
    let Some(shortcut) = script(profile) else {
        scan.problems.push(format!(
            "{}: missing, or not a script the launcher wrote",
            profile.join("shortcuts/launch-game").display()
        ));
        return scan;
    };
    for record in state.games {
        if let Some(game) = game(&record, &shortcut) {
            scan.games.push(game);
        }
    }
    scan
}

/// The script that starts a game by its id, when the launcher wrote it.
fn script(profile: &Path) -> Option<PathBuf> {
    let link = profile.join("shortcuts/launch-game");
    let second = fs::read_to_string(&link).ok()?.lines().nth(1)?.to_owned();
    second.starts_with(MARKER).then_some(link)
}

/// One game, when it is installed and can be started.
fn game(record: &Record, shortcut: &Path) -> Option<Game> {
    let id = record.id.trim();
    if id.is_empty() || record.install_state.as_deref() != Some("ready") {
        return None;
    }
    let path = PathBuf::from(record.path.as_deref()?.trim());
    if !path.is_absolute() {
        return None;
    }
    let title = match record.title.trim() {
        "" => id.to_owned(),
        title => title.to_owned(),
    };
    // The Store's product id is the one the Store page knows the game by, so
    // the library's id is its own; the launcher's id is what starts it.
    let library_id = match record.store_id.as_deref().map(str::trim) {
        Some(product) if !product.is_empty() => product.to_owned(),
        _ => id.to_owned(),
    };
    let mut game = Game::new(
        format!("xbox:{library_id}"),
        title,
        Source::Xbox,
        Launch::Command {
            program: shortcut.display().to_string(),
            args: vec![id.to_owned()],
            dir: None,
        },
    );
    game.details = Details {
        developer: record.publisher.clone(),
        platform: Some("windows".to_owned()),
        install_path: Some(path),
        ..Details::default()
    };
    // The launcher carries the cover inside its record, as a `data:` URL the
    // covers fetch knows to read; the wide art is the Store's own address.
    game.cover_url = record.cover_data_url.clone().filter(|url| !url.is_empty());
    game.hero_url = record
        .hero_url
        .clone()
        .filter(|url| url.starts_with("https://") || url.starts_with("http://"));
    Some(game)
}

/// `launcher-state.json`, as much of it as Zendo reads.
#[derive(Debug, Deserialize)]
struct State {
    #[serde(default)]
    games: Vec<Record>,
}

/// One game in the launcher's state.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    #[serde(default)]
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    publisher: Option<String>,
    #[serde(default)]
    path: Option<String>,
    /// The Store's product id, which the account's entitlements are keyed by.
    #[serde(default, rename = "storeId")]
    store_id: Option<String>,
    #[serde(default)]
    install_state: Option<String>,
    #[serde(default)]
    cover_data_url: Option<String>,
    #[serde(default)]
    hero_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A profile with `launcher-state.json` and the launcher's script.
    fn profile(name: &str, state: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "zendo-xodus-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos())
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("shortcuts")).expect("the test's folders");
        fs::write(dir.join("launcher-state.json"), state).expect("the state file");
        fs::write(
            dir.join("shortcuts/launch-game"),
            format!(
                "#!/bin/sh\n{MARKER}{}\nexec xodus-cli launch \"$@\"\n",
                dir.display()
            ),
        )
        .expect("the launcher's script");
        dir
    }

    fn by_id<'a>(scan: &'a Scan, id: &str) -> &'a Game {
        scan.games
            .iter()
            .find(|game| game.id == id)
            .expect("the game")
    }

    #[test]
    fn reads_the_games_that_are_ready_and_startable() {
        let dir = profile(
            "ready",
            r#"{
                "games": [
                    {
                        "id": "0123456789abcdef",
                        "title": "Halo Infinite",
                        "publisher": "Xbox Game Studios",
                        "storeId": "9NNX1VVR9WKB",
                        "path": "/home/me/Games/Halo Infinite",
                        "installState": "ready",
                        "executable": "HaloInfinite.exe",
                        "coverDataUrl": "data:image/png;base64,iVBORw0KGgo=",
                        "heroUrl": "https://store-images.s-microsoft.com/hero.jpg"
                    },
                    { "id": "notready", "title": "Installing", "path": "/home/me/Games/X", "installState": "installing" },
                    { "id": "relative", "title": "Relative", "path": "Games/X", "installState": "ready" },
                    { "id": "nopath", "title": "No path", "installState": "ready" }
                ]
            }"#,
        );
        let scan = scan(Some(&dir));
        assert!(scan.problems.is_empty(), "{:?}", scan.problems);
        assert_eq!(scan.games.len(), 1, "only the ready, absolute game");
        // The Store's product id names it, so the Store page knows it; the
        // launcher's id is what starts it.
        let game = by_id(&scan, "xbox:9NNX1VVR9WKB");
        assert_eq!(game.title, "Halo Infinite");
        assert_eq!(game.source, Source::Xbox);
        assert_eq!(
            game.launch,
            Launch::Command {
                program: dir.join("shortcuts/launch-game").display().to_string(),
                args: vec!["0123456789abcdef".to_owned()],
                dir: None,
            }
        );
        assert_eq!(
            game.details.install_path.as_deref(),
            Some(Path::new("/home/me/Games/Halo Infinite"))
        );
        assert_eq!(game.details.developer.as_deref(), Some("Xbox Game Studios"));
        assert_eq!(
            game.cover_url.as_deref(),
            Some("data:image/png;base64,iVBORw0KGgo=")
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_record_without_its_store_id_takes_the_launchers() {
        let dir = profile(
            "nostore",
            r#"{"games":[{"id":"aaa","title":"A","path":"/games/a","installState":"ready"}]}"#,
        );
        let scan = scan(Some(&dir));
        assert_eq!(by_id(&scan, "xbox:aaa").title, "A");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_title_that_is_empty_takes_the_id() {
        let dir = profile(
            "untitled",
            r#"{"games":[{"id":"aaa","title":"","path":"/games/a","installState":"ready"}]}"#,
        );
        let scan = scan(Some(&dir));
        assert_eq!(by_id(&scan, "xbox:aaa").title, "aaa");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_script_that_is_not_the_launchers_gives_a_problem_and_no_games() {
        let dir = profile(
            "foreign",
            r#"{"games":[{"id":"aaa","title":"A","path":"/games/a","installState":"ready"}]}"#,
        );
        fs::write(dir.join("shortcuts/launch-game"), "#!/bin/sh\nrm -rf /\n")
            .expect("another script");
        let scan = scan(Some(&dir));
        assert!(scan.games.is_empty(), "a foreign script is not started");
        assert_eq!(scan.problems.len(), 1);
        assert!(
            scan.problems[0].contains("launch-game"),
            "{:?}",
            scan.problems
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn takes_the_appimage_for_this_machine() {
        let name = "Xodus-Nexus-0.5.0-x86_64.AppImage";
        assert!(super::appimage_for(name, "x86_64"));
        assert!(!super::appimage_for(name, "aarch64"));
        assert!(super::appimage_for(
            "Xodus-Nexus-0.5.0-arm64.AppImage",
            "aarch64"
        ));
        assert!(
            super::appimage_for("Xodus-Nexus-0.5.0-aarch64.AppImage", "aarch64"),
            "either name the release gives it"
        );
        assert!(
            !super::appimage_for("Xodus-Nexus-0.5.0-x86_64.AppImage.sha512sum", "x86_64"),
            "the checksum file is not the AppImage"
        );
        assert!(!super::appimage_for(name, "riscv64"), "no AppImage here");
    }

    #[test]
    fn reads_the_entitlements_then_the_store_catalogue() {
        // The backend writes its diagnostics before the answer.
        let answer = "NEXUS_SESSION_DIAGNOSTIC_V1 ticket-response-received\n\
                      NEXUS_LIBRARY_DIAGNOSTIC_V1 complete 0 0\n\
                      {\"account\":\"me@example.com\",\"entitlements\":[\
                      {\"productId\":\"9AAA\",\"status\":\"Active\"},\
                      {\"productId\":\"9BBB\",\"status\":\"Active\"},\
                      {\"productId\":\"9CCC\",\"status\":\"Revoked\"}]}";
        let json = &answer[answer.find('{').unwrap()..];
        let library: Library = serde_json::from_str(json).unwrap();
        let ids: Vec<&str> = library
            .entitlements
            .iter()
            .filter(|entitlement| entitlement.status == "Active")
            .map(|entitlement| entitlement.product_id.as_str())
            .collect();
        assert_eq!(ids, ["9AAA", "9BBB"], "a revoked one is not owned");

        let catalogue = Catalogue {
            products: vec![
                Product {
                    id: "9AAA".into(),
                    schema: "ProductGame;1".into(),
                    localized: vec![Localized {
                        title: "Halo Infinite".into(),
                        short_title: None,
                        developer: Some("343 Industries".into()),
                        publisher: Some("Xbox Game Studios".into()),
                        short_description: Some("Spartans. ".into()),
                        description: None,
                        images: vec![
                            Art {
                                purpose: "Poster".into(),
                                uri: "//img/p".into(),
                            },
                            Art {
                                purpose: "SuperHeroArt".into(),
                                uri: "//img/h".into(),
                            },
                        ],
                    }],
                },
                Product {
                    id: "9BBB".into(),
                    schema: "ProductAddOn;3".into(),
                    localized: vec![Localized {
                        title: "Some Skins".into(),
                        short_title: None,
                        developer: None,
                        publisher: None,
                        short_description: None,
                        description: None,
                        images: vec![],
                    }],
                },
            ],
        };
        let games: Vec<Owned> = catalogue.products.iter().filter_map(owned_game).collect();
        assert_eq!(games.len(), 1, "an add-on is not a game");
        let game = &games[0];
        assert_eq!(game.id, "xbox:9AAA");
        assert_eq!(game.title, "Halo Infinite");
        assert_eq!(game.developer.as_deref(), Some("343 Industries"));
        assert_eq!(game.publisher.as_deref(), Some("Xbox Game Studios"));
        assert_eq!(game.summary.as_deref(), Some("Spartans."));
        assert_eq!(game.platforms, ["Windows"]);
        assert_eq!(game.cover_url.as_deref(), Some("https://img/p?w=600"));
        assert_eq!(game.hero_url.as_deref(), Some("https://img/h?w=1920"));
    }

    #[test]
    fn reads_a_title_id_and_orders_achievements() {
        let dir = std::env::temp_dir().join(format!(
            "zendo-xodus-title-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos())
        ));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("MicrosoftGame.config"),
            "<?xml version=\"1.0\"?>\n<Game><TitleId>61B215AE</TitleId></Game>\n",
        )
        .unwrap();
        assert_eq!(title_id(&dir), Some(1_639_060_910));
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(title_id(Path::new("/nowhere")), None, "no config, no title");

        let answer: super::Achieved = serde_json::from_str(
            r#"{"account":"me","items":[
                 {"name":"Broken","description":"Defeat The Unbreakable.","secret":false,"revoked":false,"unlockedAt":null},
                 {"name":"With Light Comes Hope","description":"Destroy the Night Beacon Base.","secret":false,"revoked":false,"unlockedAt":"2026-11-16T20:00:00.000Z"},
                 {"name":"Secret","description":"?","secret":true,"revoked":false,"unlockedAt":null},
                 {"name":"Gone","description":"","secret":false,"revoked":true,"unlockedAt":null}]}"#,
        )
        .unwrap();
        let mut list: Vec<crate::achievements::Achievement> = answer
            .items
            .into_iter()
            .filter(|item| !item.revoked)
            .map(|item| crate::achievements::Achievement {
                name: item.name,
                description: item.description,
                hidden: item.secret,
                unlocked: item.unlocked_at.as_deref().and_then(crate::time::parse_utc),
                rarity: None,
                icon: None,
            })
            .collect();
        list.sort_by(|one, other| match (one.unlocked, other.unlocked) {
            (Some(one), Some(other)) => other.cmp(&one),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => one.name.cmp(&other.name),
        });
        let names: Vec<&str> = list.iter().map(|one| one.name.as_str()).collect();
        assert_eq!(
            names,
            ["With Light Comes Hope", "Broken", "Secret"],
            "unlocked first, then the locked by name; a revoked one left out"
        );
        assert_eq!(
            list[0].unlocked,
            Some(1_794_859_200),
            "2026-11-16T20:00:00Z"
        );
        assert!(list[2].hidden, "a secret one, kept back until unlocked");
    }

    #[test]
    fn picks_the_desktop_package_and_says_whether_it_can_install() {
        let resolved: super::Resolved = serde_json::from_str(
            r#"{"choices":[
                 {"contentId":"a","downloadBytes":5506641920,"kind":"Game","packageFormat":"XVC","platform":"Windows.Xbox","supported":false},
                 {"contentId":"b","downloadBytes":5399841994,"kind":"Game","packageFormat":"EAPPX","platform":"Windows.Desktop","supported":false}],
                "market":"US","rootProductId":"9NJRX71M5X9P","schema":1,"status":"unavailable"}"#,
        )
        .unwrap();
        let picked = resolved
            .choices
            .iter()
            .find(|choice| choice.platform == "Windows.Desktop")
            .expect("the desktop build");
        assert_eq!(picked.download_bytes, Some(5_399_841_994), "the PC's build");
        assert_eq!(picked.package_format, "EAPPX");
        assert!(
            !picked.supported || resolved.status == "unavailable",
            "Cuphead is not available on a PC"
        );

        let ready: super::Resolved = serde_json::from_str(
            r#"{"choices":[{"downloadBytes":2087256064,"kind":"Game","packageFormat":"MSIXVC","platform":"Windows.Desktop","supported":true}],"status":"choice"}"#,
        )
        .unwrap();
        assert!(ready.choices[0].supported && ready.status != "unavailable");
        assert_eq!(ready.choices[0].download_bytes, Some(2_087_256_064));
    }

    #[test]
    fn a_title_that_is_empty_takes_the_short_one() {
        let product = Product {
            id: "9AAA".into(),
            schema: "ProductGame;1".into(),
            localized: vec![Localized {
                title: " ".into(),
                short_title: Some("Retro Classics".into()),
                developer: None,
                publisher: None,
                short_description: None,
                description: None,
                images: vec![],
            }],
        };
        assert_eq!(owned_game(&product).unwrap().title, "Retro Classics");
    }

    #[test]
    fn no_profile_is_no_game_and_no_problem() {
        let scan = scan(None);
        assert!(scan.games.is_empty());
        assert!(scan.problems.is_empty());
    }
}

/// The Game Pass games the account owns: the launcher's backend lists its
/// entitlements, and the Store's catalogue names them and gives their art.
///
/// # Errors
///
/// When the backend or the Store cannot be asked, or answer oddly.
pub fn owned() -> anyhow::Result<Vec<Owned>> {
    let ids = entitlements()?;
    let products = catalogue(&ids)?;
    crate::owned::keep(Shop::Xbox, products.iter().filter_map(owned_game).collect())
}

/// The product ids the account holds, from the backend's `library --json`.
fn entitlements() -> anyhow::Result<Vec<String>> {
    let answer: Library = serde_json::from_str(&backend(&["library", "--json"])?)
        .context("Nexus Launcher's backend's list was odd")?;
    Ok(answer
        .entitlements
        .iter()
        .filter(|entitlement| entitlement.status == "Active")
        .map(|entitlement| entitlement.product_id.clone())
        .collect())
}

/// The catalogue's products for those ids, a batch at a time.
fn catalogue(ids: &[String]) -> anyhow::Result<Vec<Product>> {
    let agent = crate::http::download_agent();
    let mut products = Vec::new();
    for batch in ids.chunks(BATCH) {
        let url = format!(
            "{CATALOGUE}?bigIds={}&market={MARKET}&languages=en-US&fieldsTemplate=Details",
            batch.join(",")
        );
        let body = agent
            .get(&url)
            .call()
            .context("could not reach the Microsoft Store's catalogue")?
            .into_body()
            .read_to_string()
            .context("the Store's catalogue answer was cut short")?;
        let answer: Catalogue =
            serde_json::from_str(&body).context("the Store's catalogue answer was odd")?;
        products.extend(answer.products);
    }
    Ok(products)
}

/// One owned game, when the product is a game and not an add-on.
fn owned_game(product: &Product) -> Option<Owned> {
    if !product.schema.starts_with("ProductGame") {
        return None;
    }
    let local = product.localized.first()?;
    let title = match local.title.trim() {
        "" => local.short_title.as_deref()?.trim().to_owned(),
        title => title.to_owned(),
    };
    if title.is_empty() {
        return None;
    }
    let mut game = Owned::new(Shop::Xbox, &product.id, title);
    game.developer.clone_from(&local.developer);
    game.publisher.clone_from(&local.publisher);
    game.summary = local
        .short_description
        .clone()
        .or_else(|| local.description.clone())
        .map(|summary| summary.trim().to_owned())
        .filter(|summary| !summary.is_empty());
    game.platforms = vec!["Windows".to_owned()];
    game.cover_url = local.image("Poster", 600);
    game.hero_url = local
        .image("SuperHeroArt", 1920)
        .or_else(|| local.image("TitledHeroArt", 1920));
    Some(game)
}

/// What the Store would install for a product on this machine: whether it
/// can at all, and how many bytes it would download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    /// Whether this PC can install it.
    pub supported: bool,
    /// The Store's own word for the product: `choice` when it has a package,
    /// `unavailable` when it has none for this machine.
    pub status: String,
    /// The bytes of the build picked, when the Store says.
    pub download_bytes: Option<u64>,
    /// The build picked, for the Technical card: `EAPPX`, `MSIXVC`, `XVC`.
    pub format: Option<String>,
}

/// What the Store would install for a product, from the backend's
/// `resolve-package`: this asks the Store, not the launcher, so it needs
/// nothing installed but the backend.
///
/// # Errors
///
/// When the backend is not installed, cannot be run, or answers oddly.
pub fn package(product: &str) -> anyhow::Result<Package> {
    let answer: Resolved = serde_json::from_str(&backend(&[
        "resolve-package",
        product,
        "--json",
        "--market",
        MARKET,
    ])?)
    .context("Nexus Launcher's backend's package was odd")?;
    // The desktop build is the one a PC runs; the Xbox one is for a console.
    let picked = answer
        .choices
        .iter()
        .find(|choice| choice.platform == "Windows.Desktop")
        .or_else(|| answer.choices.first());
    Ok(Package {
        supported: picked.is_some_and(|choice| choice.supported) && answer.status != "unavailable",
        status: answer.status,
        download_bytes: picked.and_then(|choice| choice.download_bytes),
        format: picked.map(|choice| choice.package_format.clone()),
    })
}

/// The Xbox title id the backend wants, in decimal: the `<TitleId>` in the
/// `MicrosoftGame.config` beside the game's executable, which is hex.
#[must_use]
pub fn title_id(install: &Path) -> Option<u64> {
    let text = fs::read_to_string(install.join("MicrosoftGame.config")).ok()?;
    let start = text.find("<TitleId>")? + "<TitleId>".len();
    let end = text[start..].find("</TitleId>")? + start;
    u64::from_str_radix(text[start..end].trim(), 16).ok()
}

/// The account's achievements for a title, as the backend gives them:
/// unlocked first, newest first, then the locked by name. No icons: the
/// backend gives addresses, and their pictures are not fetched.
///
/// # Errors
///
/// When the backend is not installed, cannot be run, or answers oddly.
pub fn achievements(title: u64) -> anyhow::Result<Vec<crate::achievements::Achievement>> {
    let answer: Achieved =
        serde_json::from_str(&backend(&["achievements", &title.to_string(), "--json"])?)
            .context("Nexus Launcher's backend's achievements were odd")?;
    let mut list: Vec<crate::achievements::Achievement> = answer
        .items
        .into_iter()
        .filter(|item| !item.revoked)
        .map(|item| crate::achievements::Achievement {
            name: item.name,
            description: item.description,
            hidden: item.secret,
            unlocked: item.unlocked_at.as_deref().and_then(crate::time::parse_utc),
            rarity: None,
            icon: None,
        })
        .collect();
    list.sort_by(|one, other| match (one.unlocked, other.unlocked) {
        (Some(one), Some(other)) => other.cmp(&one),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => one.name.cmp(&other.name),
    });
    Ok(list)
}

/// The backend's answer about a title's achievements.
#[derive(Deserialize)]
struct Achieved {
    #[serde(default)]
    items: Vec<AchievedItem>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AchievedItem {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    secret: bool,
    #[serde(default)]
    revoked: bool,
    #[serde(default)]
    unlocked_at: Option<String>,
}

/// The backend's answer about a product's packages.
#[derive(Deserialize)]
struct Resolved {
    #[serde(default)]
    status: String,
    #[serde(default)]
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Choice {
    #[serde(default)]
    platform: String,
    #[serde(default)]
    package_format: String,
    #[serde(default)]
    download_bytes: Option<u64>,
    #[serde(default)]
    supported: bool,
}

/// The backend's answer about the account's entitlements.
#[derive(Deserialize)]
struct Library {
    #[serde(default)]
    entitlements: Vec<Entitlement>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entitlement {
    product_id: String,
    #[serde(default)]
    status: String,
}

/// The catalogue's answer.
#[derive(Deserialize)]
struct Catalogue {
    #[serde(rename = "Products", default)]
    products: Vec<Product>,
}

/// One product of the catalogue.
#[derive(Deserialize)]
struct Product {
    #[serde(rename = "ProductId")]
    id: String,
    /// `ProductGame;1` for a game, `ProductAddOn;3` for an add-on.
    #[serde(rename = "ProductBSchema", default)]
    schema: String,
    #[serde(rename = "LocalizedProperties", default)]
    localized: Vec<Localized>,
}

#[derive(Deserialize)]
struct Localized {
    #[serde(rename = "ProductTitle", default)]
    title: String,
    #[serde(rename = "ShortTitle", default)]
    short_title: Option<String>,
    #[serde(rename = "DeveloperName", default)]
    developer: Option<String>,
    #[serde(rename = "PublisherName", default)]
    publisher: Option<String>,
    #[serde(rename = "ShortDescription", default)]
    short_description: Option<String>,
    #[serde(rename = "ProductDescription", default)]
    description: Option<String>,
    #[serde(rename = "Images", default)]
    images: Vec<Art>,
}

impl Localized {
    /// The address of one image, at a width, as the Store serves it; the
    /// catalogue gives it without a scheme.
    fn image(&self, purpose: &str, width: u32) -> Option<String> {
        let art = self.images.iter().find(|art| art.purpose == purpose)?;
        if art.uri.is_empty() {
            return None;
        }
        let separator = if art.uri.contains('?') { '&' } else { '?' };
        Some(format!("https:{}{separator}w={width}", art.uri))
    }
}

#[derive(Deserialize)]
struct Art {
    #[serde(rename = "ImagePurpose", default)]
    purpose: String,
    #[serde(rename = "Uri", default)]
    uri: String,
}

#[cfg(test)]
mod online {
    /// Asks GitHub; run with `cargo test -p zendo-core xodus::online -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the network"]
    fn lists_the_real_latest_release() {
        let release = super::release().unwrap();
        println!(
            "{} {} {} {}",
            release.tag, release.asset.name, release.asset.size, release.asset.sha256
        );
        assert!(super::appimage_for(
            &release.asset.name,
            std::env::consts::ARCH
        ));
        assert_eq!(release.asset.sha256.len(), 64, "a digest to check against");
    }

    /// Asks the real backend for Minecraft Legends' achievements; run with
    /// `cargo test -p zendo-core xodus::online -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the network and a signed-in Nexus Launcher"]
    fn reads_real_achievements() {
        let list = super::achievements(1_639_060_910).unwrap();
        println!("{} achievements", list.len());
        for one in list.iter().take(3) {
            println!(
                "{} | {} | unlocked {:?} | hidden {}",
                one.name, one.description, one.unlocked, one.hidden
            );
        }
        assert!(!list.is_empty(), "the title has achievements");
    }

    /// Asks the real backend what two products would install; run with
    /// `cargo test -p zendo-core xodus::online -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the network and a signed-in Nexus Launcher"]
    fn resolves_real_packages() {
        for product in ["9NBLGGH2JHXJ", "9NJRX71M5X9P"] {
            let package = super::package(product).unwrap();
            println!(
                "{product} supported={} status={} bytes={:?} format={:?}",
                package.supported, package.status, package.download_bytes, package.format
            );
            assert!(!package.status.is_empty());
            assert!(
                !package.supported || package.download_bytes.is_some(),
                "an installable build has a size"
            );
        }
    }

    /// Downloads the real `AppImage` into a throwaway home; run with
    /// `XDG_DATA_HOME=.../zendo-xodus-live/data XDG_CACHE_HOME=.../zendo-xodus-live/cache
    /// cargo test -p zendo-core xodus::online -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the network and writes to a throwaway home"]
    fn installs_the_real_launcher() {
        let data = std::env::var("XDG_DATA_HOME").unwrap_or_default();
        assert!(
            data.contains("zendo-xodus-live"),
            "set XDG_DATA_HOME and XDG_CACHE_HOME to a scratch folder named zendo-xodus-live"
        );
        let release = super::release().unwrap();
        let stop = std::sync::atomic::AtomicBool::new(false);
        let at = super::install(&release, &stop, &mut |_, _| {}).unwrap();
        assert_eq!(super::installed(), Some(at.clone()));
        let bytes = std::fs::read(&at).unwrap();
        assert!(bytes.starts_with(b"\x7fELF"), "an AppImage is an ELF file");
        let mode =
            std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&at).unwrap().permissions());
        assert_eq!(mode & 0o111, 0o111, "it can be run");
        println!(
            "{} {} -> {} bytes",
            release.tag,
            release.asset.name,
            bytes.len()
        );
    }
    /// Asks the real backend and the Store for the account's games; run with
    /// `cargo test -p zendo-core xodus::online -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the network and a signed-in Nexus Launcher"]
    fn lists_the_owned_games() {
        let games = super::owned().unwrap();
        println!("{} games", games.len());
        for game in games.iter().take(5) {
            println!(
                "{} | {} | {} | cover {}",
                game.id,
                game.title,
                game.publisher.as_deref().unwrap_or(""),
                game.cover_url.is_some()
            );
        }
        assert!(!games.is_empty(), "the account owns a game");
        assert!(games.iter().all(|game| game.id.starts_with("xbox:")));
        assert!(games.iter().all(|game| !game.title.is_empty()));
        assert!(
            games.iter().any(|game| game.cover_url.is_some()),
            "art found"
        );
    }
}
