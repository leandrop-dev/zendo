//! Valve's Steam Linux Runtimes: the containers Steam runs games in, each
//! a tool in a Steam library (`steamapps/common/SteamLinuxRuntime_*`).
//! Zendo runs a native game in one when its launch options ask (older
//! games built for scout, which Steam runs in scout on soldier), and runs
//! Proton in 4.0 when the options leave umu out, as both Proton builds
//! here ask for it (`require_tool_appid` 4183110). umu keeps its own copy
//! of 4.0 (`~/.local/share/umu/steamrt4`), used when Steam's is missing.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{paths, steam};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Runtime {
    /// 1.0, run inside soldier.
    Scout,
    /// 2.0.
    Soldier,
    /// 3.0.
    Sniper,
    /// 4.0.
    #[serde(rename = "steamrt4")]
    Four,
}

impl Runtime {
    pub const ALL: [Self; 4] = [Self::Scout, Self::Soldier, Self::Sniper, Self::Four];

    /// The name Steam lists it by.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Scout => "Steam Linux Runtime 1.0 (scout)",
            Self::Soldier => "Steam Linux Runtime 2.0 (soldier)",
            Self::Sniper => "Steam Linux Runtime 3.0 (sniper)",
            Self::Four => "Steam Linux Runtime 4.0",
        }
    }

    fn folder(self) -> &'static str {
        match self {
            Self::Scout => "SteamLinuxRuntime",
            Self::Soldier => "SteamLinuxRuntime_soldier",
            Self::Sniper => "SteamLinuxRuntime_sniper",
            Self::Four => "SteamLinuxRuntime_4",
        }
    }

    fn entry_point(self) -> &'static str {
        match self {
            Self::Scout => "scout-on-soldier-entry-point-v2",
            _ => "_v2-entry-point",
        }
    }

    /// Its folder, when it is installed with what it needs (scout needs
    /// soldier, whose container it runs in).
    #[must_use]
    pub fn find(self) -> Option<PathBuf> {
        if self == Self::Scout {
            Self::Soldier.find()?;
        }
        let mut folders: Vec<PathBuf> = steam::roots()
            .into_iter()
            .map(|root| root.join("steamapps/common").join(self.folder()))
            .collect();
        if self == Self::Four
            && let Some(data) = paths::data_home()
        {
            folders.push(data.join("umu/steamrt4"));
        }
        folders
            .into_iter()
            .find(|folder| folder.join(self.entry_point()).is_file())
    }

    /// The words that run a program inside it, from its folder: scout's
    /// own entry point, or the others' with Steam's verb for a game.
    #[must_use]
    pub fn words(self, folder: &std::path::Path) -> Vec<String> {
        let entry = folder
            .join(self.entry_point())
            .to_string_lossy()
            .into_owned();
        match self {
            Self::Scout => vec![entry, "--".into()],
            _ => vec![entry, "--verb=waitforexitandrun".into(), "--".into()],
        }
    }
}

/// The runtimes installed.
#[must_use]
pub fn installed() -> Vec<Runtime> {
    Runtime::ALL
        .into_iter()
        .filter(|runtime| runtime.find().is_some())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_a_program_with_steams_verb_but_scout_with_its_own() {
        let folder = std::path::Path::new("/steam/steamapps/common/SteamLinuxRuntime_4");
        assert_eq!(
            Runtime::Four.words(folder),
            [
                "/steam/steamapps/common/SteamLinuxRuntime_4/_v2-entry-point",
                "--verb=waitforexitandrun",
                "--"
            ]
        );
        let scout = std::path::Path::new("/s/SteamLinuxRuntime");
        assert_eq!(
            Runtime::Scout.words(scout),
            ["/s/SteamLinuxRuntime/scout-on-soldier-entry-point-v2", "--"]
        );
        let text =
            toml::to_string(&std::collections::BTreeMap::from([("r", Runtime::Four)])).unwrap();
        assert_eq!(text.trim(), "r = \"steamrt4\"");
    }

    /// Lists this computer's: `cargo test -p zendo-core steam_runtime -- --ignored`.
    #[test]
    #[ignore = "reads this computer's Steam libraries"]
    fn lists_this_computers_runtimes() {
        for runtime in Runtime::ALL {
            eprintln!("{}: {:?}", runtime.label(), runtime.find());
        }
    }
}
