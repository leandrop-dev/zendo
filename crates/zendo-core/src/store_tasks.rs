//! The other ways a store lists to start a game, besides its own: GOG's
//! play tasks (`goggame-<id>.info` in the game's folder, which gogdl runs
//! by their place in the list, `--prefer-task`), and Epic's extra launch
//! options (`extraLaunchOption_NNN_Name` and `_Args` in the game's
//! metadata, which legendary keeps; their arguments are passed to the
//! game). A game's launch options keep the one chosen by its key. Also
//! whether Epic approves a game for offline play (`CanRunOffline`).

use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use crate::stores::Store;
use crate::{Game, Launch, Source};

/// A way to start the game the store lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreTask {
    /// What the launch options keep: GOG's task's file, Epic's option's
    /// name.
    pub key: String,
    pub name: String,
    /// The file it runs (GOG) or the arguments it adds (Epic).
    pub detail: String,
}

/// What the task asks of the engine when the game starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chosen {
    /// gogdl's `--prefer-task`: its place among all the play tasks.
    Index(usize),
    /// Arguments for the game.
    Arguments(String),
}

/// The tasks a store game lists besides the one it starts with; none for
/// other games, or when the store's record cannot be read.
#[must_use]
pub fn listed(game: &Game) -> Vec<StoreTask> {
    let Launch::Engine { app, .. } = &game.launch else {
        return Vec::new();
    };
    match game.source {
        Source::Gog => game
            .details
            .install_path
            .as_deref()
            .and_then(|folder| gog_tasks(folder, app))
            .map(|tasks| other_gog_tasks(&tasks))
            .unwrap_or_default(),
        Source::Epic => epic_metadata(app)
            .map(|metadata| epic_options(&metadata))
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// What the task kept as `key` asks of the engine; `None` when the store
/// no longer lists it.
#[must_use]
pub fn chosen(game: &Game, key: &str) -> Option<Chosen> {
    let Launch::Engine { app, .. } = &game.launch else {
        return None;
    };
    match game.source {
        Source::Gog => {
            let tasks = gog_tasks(game.details.install_path.as_deref()?, app)?;
            tasks
                .iter()
                .position(|task| task.path.as_deref() == Some(key))
                .map(Chosen::Index)
        }
        Source::Epic => epic_options(&epic_metadata(app)?)
            .into_iter()
            .find(|task| task.key == key)
            .map(|task| Chosen::Arguments(task.detail)),
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlayTask {
    #[serde(default)]
    name: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    is_primary: bool,
}

/// Every play task the game's info file lists, in its order: gogdl counts
/// them all.
fn gog_tasks(folder: &Path, app: &str) -> Option<Vec<PlayTask>> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Info {
        #[serde(default)]
        play_tasks: Vec<PlayTask>,
    }
    let text = fs::read_to_string(folder.join(format!("goggame-{app}.info"))).ok()?;
    let info: Info = serde_json::from_str(&text).ok()?;
    Some(info.play_tasks)
}

/// The programs besides the primary one; links (`URLTask`) left out.
fn other_gog_tasks(tasks: &[PlayTask]) -> Vec<StoreTask> {
    tasks
        .iter()
        .filter(|task| !task.is_primary && task.kind == "FileTask")
        .filter_map(|task| {
            let path = task.path.clone()?;
            Some(StoreTask {
                key: path.clone(),
                name: task.name.trim().to_owned(),
                detail: path.replace('\\', "/"),
            })
        })
        .collect()
}

/// Whether Epic approves the game for offline play; `None` when it does
/// not say or the game is not Epic's. legendary only warns when it is
/// not.
#[must_use]
pub fn epic_offline(game: &Game) -> Option<bool> {
    let Launch::Engine { app, .. } = &game.launch else {
        return None;
    };
    if game.source != Source::Epic {
        return None;
    }
    let metadata = epic_metadata(app)?;
    let value = metadata
        .pointer("/metadata/customAttributes/CanRunOffline/value")?
        .as_str()?;
    Some(value.eq_ignore_ascii_case("true"))
}

fn epic_metadata(app: &str) -> Option<Value> {
    let file = Store::Epic
        .dir()?
        .join("metadata")
        .join(format!("{app}.json"));
    serde_json::from_slice(&fs::read(file).ok()?).ok()
}

/// `extraLaunchOption_001_Name` with its `_Args`, in their numbers' order.
fn epic_options(metadata: &Value) -> Vec<StoreTask> {
    let Some(attributes) = metadata
        .pointer("/metadata/customAttributes")
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };
    let value = |key: &str| {
        attributes
            .get(key)
            .and_then(|attribute| attribute.get("value"))
            .and_then(Value::as_str)
            .map(str::trim)
    };
    let mut numbers: Vec<&str> = attributes
        .keys()
        .filter_map(|key| {
            key.strip_prefix("extraLaunchOption_")?
                .strip_suffix("_Name")
        })
        .collect();
    numbers.sort_unstable();
    numbers
        .into_iter()
        .filter_map(|number| {
            let name = value(&format!("extraLaunchOption_{number}_Name"))?;
            let arguments = value(&format!("extraLaunchOption_{number}_Args")).unwrap_or("");
            Some(StoreTask {
                key: name.to_owned(),
                name: name.to_owned(),
                detail: arguments.to_owned(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lists_gogs_other_play_tasks_by_their_file() {
        let info = r#"{"playTasks":[
            {"category":"launcher","isPrimary":true,"name":"Cyberpunk 2077","path":"REDprelauncher.exe","type":"FileTask"},
            {"category":"game","isHidden":true,"name":"Cyberpunk 2077","path":"bin\\x64\\Cyberpunk2077.exe","type":"FileTask"},
            {"category":"document","name":"Manual","link":"https://example.com","type":"URLTask"}]}"#;
        let dir = std::env::temp_dir().join(format!("zendo-tasks-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("goggame-1423049311.info"), info).unwrap();
        let mut game = Game::new(
            "gog:1423049311".into(),
            "Cyberpunk 2077".into(),
            Source::Gog,
            Launch::Engine {
                runner: "gogdl".into(),
                app: "1423049311".into(),
            },
        );
        game.details.install_path = Some(dir.clone());
        let tasks = listed(&game);
        assert_eq!(
            tasks,
            [StoreTask {
                key: "bin\\x64\\Cyberpunk2077.exe".into(),
                name: "Cyberpunk 2077".into(),
                detail: "bin/x64/Cyberpunk2077.exe".into(),
            }]
        );
        // gogdl counts every task, the primary one too.
        assert_eq!(
            chosen(&game, "bin\\x64\\Cyberpunk2077.exe"),
            Some(Chosen::Index(1))
        );
        assert_eq!(chosen(&game, "gone.exe"), None);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reads_epics_extra_launch_options_in_order() {
        let metadata = json!({"metadata": {"customAttributes": {
            "extraLaunchOption_002_Name": {"type": "STRING", "value": "Safe mode"},
            "extraLaunchOption_002_Args": {"type": "STRING", "value": "-safe"},
            "extraLaunchOption_001_Name": {"type": "STRING", "value": "Launch without Anti-Cheat"},
            "extraLaunchOption_001_Args": {"type": "STRING", "value": "-noeac"},
            "AdditionalCommandLine": {"type": "STRING", "value": "-epicapp=Fortnite"}
        }}});
        let options = epic_options(&metadata);
        let names: Vec<(&str, &str)> = options
            .iter()
            .map(|task| (task.name.as_str(), task.detail.as_str()))
            .collect();
        assert_eq!(
            names,
            [
                ("Launch without Anti-Cheat", "-noeac"),
                ("Safe mode", "-safe")
            ]
        );
        assert_eq!(epic_options(&json!({"metadata": {}})).len(), 0);
    }
}
