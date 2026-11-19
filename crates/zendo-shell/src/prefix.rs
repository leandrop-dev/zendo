//! What a game's Wine prefix card says without a toolkit: what a tool is
//! called while it runs, and how a components run went. Both windows use
//! these, so the words stay one.

use zendo_core::prefix_tools::{Ran, Tool};

use crate::t;

/// What the card says runs.
#[must_use]
pub fn label(tool: &Tool) -> String {
    match tool {
        Tool::WineSettings => t!("prefix-wine-settings"),
        Tool::Winetricks => "winetricks".to_owned(),
        Tool::Components(verbs) => verbs.join(" "),
        Tool::GogSetup => t!("prefix-gog-setup"),
        Tool::KnownFixes => t!("prefix-known-fixes"),
        Tool::Program(program) => program
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned()),
    }
}

/// What a components run did: what it installed, and what was there.
#[must_use]
pub fn installed(ran: &Ran) -> String {
    match (ran.installed.is_empty(), ran.already.is_empty()) {
        (_, true) => t!("prefix-installed", components = ran.installed.join(" ")),
        (true, false) => t!("prefix-all-there", components = ran.already.join(" ")),
        (false, false) => t!(
            "prefix-installed-some",
            components = ran.installed.join(" "),
            already = ran.already.join(" ")
        ),
    }
}

/// The line a finished tool leaves, and whether it went well: what a
/// components run installed, else that the tool ended, or why it failed.
#[must_use]
pub fn done(tool: &Tool, result: Result<Ran, String>) -> (String, bool) {
    match (tool, result) {
        (Tool::Components(_), Ok(ran)) => (installed(&ran), true),
        (tool, Ok(_)) => (t!("prefix-done", what = label(tool)), true),
        (_, Err(err)) => (err, false),
    }
}

#[cfg(test)]
mod tests {
    use zendo_core::prefix_tools::Tool;

    use super::*;

    #[test]
    fn says_what_a_components_run_did() {
        let ran = Ran {
            installed: vec!["vcrun2022".into()],
            already: vec!["d3dx9".into()],
        };
        assert_eq!(
            installed(&ran),
            "Installed vcrun2022; d3dx9 was there already"
        );
        assert_eq!(
            label(&Tool::Components(vec!["a".into(), "b".into()])),
            "a b"
        );
        assert_eq!(label(&Tool::WineSettings), "Wine settings");
        let failed: Result<Ran, String> = Err("no Proton".into());
        assert_eq!(done(&Tool::GogSetup, failed), ("no Proton".into(), false));
    }
}
