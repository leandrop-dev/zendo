//! GOG's setup for a Windows game Zendo installed from GOG, as Heroic runs
//! it on Linux the first time a game's prefix is made: the
//! redistributables the game lists (Visual C++, DirectX, `PhysX`…)
//! installed into its prefix, then the game's own install steps (GOG's
//! script interpreter, or the installer a product names), which write
//! registry keys some games need. Done once per game and prefix, before
//! its first start, and again on asking (the Wine prefix card).
//!
//! What is read (gogdl 1.x in Heroic 2.22.4, and Heroic's own GOG setup,
//! for facts only):
//! - The game's manifest, `heroic_gogdl/manifests/<id>` in Zendo's gogdl
//!   folder: `dependencies` (version 2) or each depot's `redist`
//!   (version 1), `scriptInterpreter`, and the `products` with their
//!   `temp_executable`.
//! - `gogdl redist --ids <ids> --path <folder>` downloads the shared
//!   redistributables (paths starting `__redist`) into
//!   `$XDG_DATA_HOME/zendo/gog-redist`, with GOG's list of them in
//!   `.gogdl-redist-manifest` there: each one's `executable` `path` and
//!   `arguments`. gogdl removes what it was not asked for, so it is always
//!   asked for every GOG game's, and the script interpreter (`ISI`).
//! - An `.msi` (`PhysX` Legacy) goes through `msiexec /i … /qb`.

use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use crate::stores::Store;
use crate::{Game, Launch, Source, launch, paths};

/// The script interpreter's id among GOG's redistributables.
const ISI: &str = "ISI";

/// The games whose setup ran in a prefix, kept in it.
const DONE_FILE: &str = ".zendo-gog-setup.json";

/// What a game's manifest says of its setup.
#[derive(Debug, Default, Deserialize)]
struct Manifest {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default, rename = "scriptInterpreter")]
    script_interpreter: bool,
    #[serde(default)]
    products: Vec<Product>,
    /// Version 1's: each depot may name a redistributable.
    #[serde(default)]
    product: Option<V1Product>,
}

#[derive(Debug, Default, Deserialize)]
struct Product {
    #[serde(rename = "productId")]
    id: String,
    #[serde(default)]
    temp_executable: String,
}

#[derive(Debug, Default, Deserialize)]
struct V1Product {
    #[serde(default)]
    depots: Vec<V1Depot>,
}

#[derive(Debug, Default, Deserialize)]
struct V1Depot {
    redist: Option<String>,
}

impl Manifest {
    /// The redistributables the game needs, in the manifest's order.
    fn redistributables(&self) -> Vec<String> {
        let listed: Vec<String> = if self.version == 1 {
            self.product
                .iter()
                .flat_map(|product| &product.depots)
                .filter_map(|depot| depot.redist.clone())
                .collect()
        } else {
            self.dependencies.clone()
        };
        let mut seen = BTreeSet::new();
        listed
            .into_iter()
            .filter(|id| seen.insert(id.clone()))
            .collect()
    }
}

/// GOG's list of redistributables, as gogdl keeps it.
#[derive(Debug, Default, Deserialize)]
struct Repository {
    #[serde(default)]
    depots: Vec<Redist>,
}

#[derive(Debug, Clone, Deserialize)]
struct Redist {
    #[serde(rename = "dependencyId")]
    id: String,
    #[serde(rename = "readableName", default)]
    name: String,
    executable: Executable,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct Executable {
    #[serde(default)]
    path: String,
    #[serde(default)]
    arguments: String,
}

/// What Zendo's gogdl records of an installed game.
#[derive(Debug, Default, Deserialize)]
struct Install {
    #[serde(rename = "appName")]
    app: String,
    #[serde(rename = "buildId", default)]
    build: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    language: String,
    #[serde(rename = "installedDLCs", default)]
    dlc: Vec<String>,
}

/// How the setup went: each step that failed, with why.
#[derive(Debug, Default)]
pub struct Setup {
    pub failed: Vec<String>,
}

/// Whether the game is a Windows one Zendo installed from GOG.
#[must_use]
pub fn applies(game: &Game) -> bool {
    game.source == Source::Gog
        && matches!(&game.launch, Launch::Engine { runner, .. } if runner == "gogdl")
        && launch::needs_proton(game)
}

/// Whether GOG's setup is still to run in the game's prefix: it has a
/// manifest and its prefix has not had it.
#[must_use]
pub fn needed(game: &Game) -> bool {
    let Launch::Engine { app, .. } = &game.launch else {
        return false;
    };
    applies(game)
        && manifest_path(app).is_some_and(|path| path.is_file())
        && launch::prefix(game).is_some_and(|prefix| !done_in(&prefix).contains(app))
}

/// Has GOG's setup run again at the game's next start, as Heroic does
/// after a move: the game's own steps write its folder into the prefix.
///
/// # Errors
///
/// When the prefix's record cannot be written.
pub fn forget(game: &Game) -> anyhow::Result<()> {
    let Launch::Engine { app, .. } = &game.launch else {
        return Ok(());
    };
    let Some(prefix) = launch::prefix(game).filter(|_| applies(game)) else {
        return Ok(());
    };
    let mut done = done_in(&prefix);
    if !done.remove(app) {
        return Ok(());
    }
    let file = prefix.join(DONE_FILE);
    paths::write_atomic(&file, &serde_json::to_vec_pretty(&done)?)
        .with_context(|| format!("could not write {}", file.display()))
}

/// Runs GOG's setup in the game's prefix, saying each step to `report`
/// (a redistributable's name, or the game's title for its own steps).
/// A step that fails does not stop the others; the prefix then counts it
/// as done all the same, so a game is not held up at every start, and
/// the Wine prefix card runs it again.
///
/// # Errors
///
/// When the game's manifest cannot be read, or the redistributables
/// cannot be downloaded.
pub fn run(game: &Game, report: &mut dyn FnMut(&str)) -> anyhow::Result<Setup> {
    let Launch::Engine { app, .. } = &game.launch else {
        bail!("Zendo did not install {} from GOG", game.title);
    };
    if !applies(game) {
        bail!("Zendo did not install {} from GOG", game.title);
    }
    let manifest = read_manifest(app)?;
    let install = install_of(app).unwrap_or_default();
    let folder = redist_folder().context("no home folder for GOG's redistributables")?;
    let wanted = manifest.redistributables();
    let mut setup = Setup::default();
    // The list comes with the redistributables. A game that needs neither
    // downloads nothing, and its own steps run without one.
    let downloaded = !wanted.is_empty() || manifest.script_interpreter;
    if downloaded {
        report("GOG's redistributables");
        download(&folder)?;
    }
    let repository = if downloaded {
        read_repository(&folder)?
    } else {
        Repository::default()
    };
    let log = log_path(&game.id).context("no folder for the log")?;
    start_log(&log)?;
    for id in &wanted {
        let Some(redist) = repository.depots.iter().find(|redist| &redist.id == id) else {
            setup
                .failed
                .push(format!("{id}: GOG lists no such redistributable"));
            continue;
        };
        // An empty path is a redistributable with nothing to run (a
        // configurator); one outside `__redist` came with the game.
        if !redist.executable.path.starts_with("__redist") {
            continue;
        }
        let name = if redist.name.is_empty() {
            id.clone()
        } else {
            redist.name.clone()
        };
        report(&name);
        let program = folder.join(&redist.executable.path);
        if !program.is_file() {
            setup
                .failed
                .push(format!("{name}: gogdl did not download it"));
            continue;
        }
        let mut command = launch::prefix_command(game, None)?;
        command
            .args(redist_words(&program, &redist.executable.arguments))
            .current_dir(&folder);
        if let Err(err) = wait(command, &log) {
            setup.failed.push(format!("{name}: {err:#}"));
        }
    }
    report(&game.title);
    for (program, args) in own_steps(game, &manifest, &install, &folder) {
        let mut command = launch::prefix_command(game, None)?;
        command.arg(&program).args(args);
        if let Some(dir) = &game.details.install_path {
            command.current_dir(dir);
        }
        if let Err(err) = wait(command, &log) {
            let name = program.file_name().unwrap_or_default().to_string_lossy();
            setup.failed.push(format!("{name}: {err:#}"));
        }
    }
    if let Some(prefix) = launch::prefix(game) {
        mark_done(&prefix, app)?;
    }
    Ok(setup)
}

/// Where the game's setup was last logged.
#[must_use]
pub fn log_path(id: &str) -> Option<PathBuf> {
    launch::log_path(id).map(|log| log.with_extension("gog-setup.log"))
}

/// The game's own steps: GOG's script interpreter for the game and each
/// DLC installed, when the manifest asks for it; otherwise the installer
/// each product names, when gogdl kept it with the support files.
fn own_steps(
    game: &Game,
    manifest: &Manifest,
    install: &Install,
    folder: &Path,
) -> Vec<(PathBuf, Vec<String>)> {
    let Launch::Engine { app, .. } = &game.launch else {
        return Vec::new();
    };
    let support = Store::Gog
        .dir()
        .map(|dir| dir.join("support").join(app))
        .unwrap_or_default();
    let game_dir = game
        .details
        .install_path
        .as_deref()
        .map(wine_path)
        .unwrap_or_default();
    let products = manifest
        .products
        .iter()
        .filter(|product| &product.id == app || install.dlc.contains(&product.id));
    let common = |product: &Product, language: &str| {
        vec![
            "/VERYSILENT".to_owned(),
            format!("/DIR={game_dir}"),
            format!("/Language={language}"),
            format!("/LANG={language}"),
            format!("/ProductId={}", product.id),
            "/galaxyclient".to_owned(),
            format!("/buildId={}", install.build),
            format!("/versionName={}", install.version),
            format!("/lang-code={}", lang_code(install)),
        ]
    };
    // GOG's installers know both spellings; one is GOG's typo.
    let shortcuts = ["/nodesktopshorctut", "/nodesktopshortcut"].map(str::to_owned);
    let language = LANGUAGE;
    if manifest.script_interpreter {
        let isi = folder.join("__redist/ISI/scriptinterpreter.exe");
        if !isi.is_file() {
            return Vec::new();
        }
        return products
            .map(|product| {
                let mut args = common(product, language);
                args.push(format!("/supportDir={}", wine_path(&support)));
                args.extend(shortcuts.clone());
                (isi.clone(), args)
            })
            .collect();
    }
    products
        .filter(|product| !product.temp_executable.is_empty())
        .filter_map(|product| {
            let program = support.join(&product.id).join(&product.temp_executable);
            program.is_file().then(|| {
                let mut args = common(product, &language.to_lowercase());
                args.extend(shortcuts.clone());
                (program, args)
            })
        })
        .collect()
}

/// The installer and its arguments, split as a shell would (GOG's are
/// mostly flags; Visual C++ 2005's carry quotes); an `.msi` through
/// `msiexec`, as Heroic runs `PhysX` Legacy.
fn redist_words(program: &Path, arguments: &str) -> Vec<String> {
    let path = program.to_string_lossy().into_owned();
    let mut words = split(arguments);
    let msi = program
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("msi"));
    if msi {
        let mut all = vec!["msiexec".to_owned(), "/i".to_owned(), path];
        all.append(&mut words);
        all.push("/qb".to_owned());
        all
    } else {
        words.insert(0, path);
        words
    }
}

/// Words split at spaces, with what is inside single or double quotes
/// kept together (the quotes dropped), as a POSIX shell does without
/// its backslashes.
fn split(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut quote: Option<char> = None;
    for c in text.chars() {
        match (quote, c) {
            (Some(open), c) if c == open => quote = None,
            (Some(_), c) => word.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                started = true;
            }
            (None, c) if c.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            (None, c) => {
                word.push(c);
                started = true;
            }
        }
    }
    if started {
        words.push(word);
    }
    words
}

/// A Linux path as Wine sees it through its `Z:` drive.
fn wine_path(path: &Path) -> String {
    format!("Z:{}", path.to_string_lossy().replace('/', "\\"))
}

/// The install's language as GOG's installers name it: Zendo installs
/// in English (`downloads::GOG_LANGUAGE`).
const LANGUAGE: &str = "English";

fn lang_code(install: &Install) -> String {
    if install.language.is_empty() {
        "en-US".to_owned()
    } else {
        install.language.clone()
    }
}

/// Every GOG game's redistributables, and the script interpreter.
fn every_needed() -> BTreeSet<String> {
    let mut ids = BTreeSet::from([ISI.to_owned()]);
    let Some(dir) = manifests_dir() else {
        return ids;
    };
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        if let Ok(manifest) = read_manifest_at(&entry.path()) {
            ids.extend(manifest.redistributables());
        }
    }
    ids
}

/// Asks gogdl for every GOG game's redistributables; it fetches only what
/// changed.
fn download(folder: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(folder).with_context(|| format!("could not make {}", folder.display()))?;
    let ids = every_needed().into_iter().collect::<Vec<_>>().join(",");
    let folder = folder.to_string_lossy();
    let workers = crate::config::Config::load_file()
        .map(|config| config.workers_args())
        .unwrap_or_default();
    let mut args = vec!["redist", "--ids", &ids, "--path", &folder];
    args.extend(workers.iter().map(String::as_str));
    Store::Gog
        .run_checked(&args)
        .context("could not download GOG's redistributables")?;
    Ok(())
}

fn read_repository(folder: &Path) -> anyhow::Result<Repository> {
    let file = folder.join(".gogdl-redist-manifest");
    let text = fs::read_to_string(&file).with_context(|| {
        format!(
            "gogdl left no list of GOG's redistributables in {}",
            folder.display()
        )
    })?;
    serde_json::from_str(&text).with_context(|| format!("{} is not valid", file.display()))
}

fn manifests_dir() -> Option<PathBuf> {
    Store::Gog
        .dir()
        .map(|dir| dir.join("heroic_gogdl/manifests"))
}

fn manifest_path(app: &str) -> Option<PathBuf> {
    manifests_dir().map(|dir| dir.join(app))
}

fn read_manifest(app: &str) -> anyhow::Result<Manifest> {
    let path = manifest_path(app).context("no home folder")?;
    read_manifest_at(&path)
}

fn read_manifest_at(path: &Path) -> anyhow::Result<Manifest> {
    let text =
        fs::read_to_string(path).with_context(|| format!("could not read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("{} is not valid", path.display()))
}

fn install_of(app: &str) -> Option<Install> {
    #[derive(Deserialize)]
    struct Installed {
        #[serde(default)]
        installed: Vec<Install>,
    }
    let file = Store::Gog.dir()?.join("installed.json");
    let installed: Installed = serde_json::from_slice(&fs::read(file).ok()?).ok()?;
    installed
        .installed
        .into_iter()
        .find(|install| install.app == app)
}

fn redist_folder() -> Option<PathBuf> {
    paths::data_home().map(|dir| dir.join("zendo/gog-redist"))
}

/// The games whose setup ran in the prefix.
fn done_in(prefix: &Path) -> BTreeSet<String> {
    fs::read(prefix.join(DONE_FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn mark_done(prefix: &Path, app: &str) -> anyhow::Result<()> {
    #[derive(Serialize)]
    struct Done<'a>(&'a BTreeSet<String>);
    fs::create_dir_all(prefix).with_context(|| format!("could not make {}", prefix.display()))?;
    let file = prefix.join(DONE_FILE);
    // Read again under the lock: a second start may be recording another
    // game's setup in the same prefix.
    paths::locked(&file, || {
        let mut done = done_in(prefix);
        done.insert(app.to_owned());
        paths::write_atomic(&file, &serde_json::to_vec_pretty(&Done(&done))?)
            .with_context(|| format!("could not write {}", file.display()))
    })
}

fn start_log(log: &Path) -> anyhow::Result<()> {
    if let Some(dir) = log.parent() {
        fs::create_dir_all(dir).with_context(|| format!("could not make {}", dir.display()))?;
    }
    File::create(log).with_context(|| format!("could not write {}", log.display()))?;
    Ok(())
}

/// Runs a step, adding its output to the setup's log, and waits for it.
fn wait(mut command: Command, log: &Path) -> anyhow::Result<()> {
    let mut file = OpenOptions::new()
        .append(true)
        .open(log)
        .with_context(|| format!("could not write {}", log.display()))?;
    writeln!(file, "\n{}\n", launch::describe(&command))?;
    let status = command
        .stdin(Stdio::null())
        .stdout(Stdio::from(file.try_clone()?))
        .stderr(Stdio::from(file))
        .status()
        .context("could not start umu-run")?;
    if !status.success() {
        bail!("it ended with {status}; see {}", log.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_each_manifest_version_needs() {
        let v2: Manifest = serde_json::from_str(
            r#"{"version": 2, "dependencies": ["MSVC2015_x64", "MSVC2015", "MSVC2015_x64"],
                "scriptInterpreter": true,
                "products": [{"productId": "1423049311", "temp_executable": ""}]}"#,
        )
        .unwrap();
        assert_eq!(v2.redistributables(), ["MSVC2015_x64", "MSVC2015"]);
        assert!(v2.script_interpreter);
        let v1: Manifest = serde_json::from_str(
            r#"{"version": 1, "product": {"depots": [{"redist": "DirectX"}, {"size": 3}, {"redist": "DirectX"}]}}"#,
        )
        .unwrap();
        assert_eq!(v1.redistributables(), ["DirectX"]);
    }

    #[test]
    fn splits_gogs_arguments_as_a_shell_would() {
        assert_eq!(
            split("/install /quiet  /norestart"),
            ["/install", "/quiet", "/norestart"]
        );
        assert_eq!(split(""), Vec::<String>::new());
        // Visual C++ 2005's, as GOG lists it.
        assert_eq!(
            split(r#"/q:a /c:"VCREDI~3.EXE /q:a /c:""msiexec /i vcredist.msi /qb"" ""#),
            [
                "/q:a",
                "/c:VCREDI~3.EXE /q:a /c:msiexec /i vcredist.msi /qb "
            ]
        );
        assert_eq!(split(r#"a "" b"#), ["a", "", "b"]);
    }

    #[test]
    fn runs_an_msi_through_msiexec_quietly() {
        assert_eq!(
            redist_words(
                Path::new("/r/__redist/PHYSXLEGACY/PhysX.msi"),
                "/passive /norestart"
            ),
            [
                "msiexec",
                "/i",
                "/r/__redist/PHYSXLEGACY/PhysX.msi",
                "/passive",
                "/norestart",
                "/qb"
            ]
        );
        assert_eq!(
            redist_words(Path::new("/r/vc.exe"), "/q /norestart"),
            ["/r/vc.exe", "/q", "/norestart"]
        );
    }

    #[test]
    fn gives_wine_paths_on_its_z_drive() {
        assert_eq!(
            wine_path(Path::new("/home/me/Games/Cyberpunk 2077")),
            r"Z:\home\me\Games\Cyberpunk 2077"
        );
    }

    #[test]
    fn keeps_the_games_set_up_in_a_prefix() {
        let dir = std::env::temp_dir().join(format!("zendo-gog-setup-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert!(done_in(&dir).is_empty());
        mark_done(&dir, "1").unwrap();
        mark_done(&dir, "2").unwrap();
        assert_eq!(
            done_in(&dir),
            BTreeSet::from(["1".to_owned(), "2".to_owned()])
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn runs_the_script_interpreter_for_the_game_and_its_dlc() {
        let folder = std::env::temp_dir().join(format!("zendo-gog-isi-{}", std::process::id()));
        let isi = folder.join("__redist/ISI/scriptinterpreter.exe");
        fs::create_dir_all(isi.parent().unwrap()).unwrap();
        fs::write(&isi, b"").unwrap();
        let mut game = Game::new(
            "gog:1".into(),
            "Sugar".into(),
            Source::Gog,
            Launch::Engine {
                runner: "gogdl".into(),
                app: "1".into(),
            },
        );
        game.details.install_path = Some("/g/Sugar".into());
        let manifest: Manifest = serde_json::from_str(
            r#"{"version": 2, "scriptInterpreter": true, "products": [
                {"productId": "1"}, {"productId": "2"}, {"productId": "3"}]}"#,
        )
        .unwrap();
        let install = Install {
            app: "1".into(),
            build: "60152385347545148".into(),
            version: "2.5".into(),
            language: "en-US".into(),
            dlc: vec!["3".into()],
        };
        let steps = own_steps(&game, &manifest, &install, &folder);
        let products: Vec<&str> = steps.iter().map(|(_, args)| args[4].as_str()).collect();
        assert_eq!(
            products,
            ["/ProductId=1", "/ProductId=3"],
            "DLC 2 is not installed"
        );
        let (program, args) = &steps[0];
        assert_eq!(program, &isi);
        assert_eq!(args[0], "/VERYSILENT");
        assert_eq!(args[1], r"/DIR=Z:\g\Sugar");
        assert!(args.contains(&"/buildId=60152385347545148".to_owned()));
        assert!(args.iter().any(|arg| arg.starts_with("/supportDir=Z:")));
        assert_eq!(args.last().map(String::as_str), Some("/nodesktopshortcut"));
        let _ = fs::remove_dir_all(&folder);
    }
}
