use std::process::Command;
use std::time::Duration;

use super::add::archive_name;
use super::*;
use crate::Source;

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("zendo-mods-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("game")).unwrap();
        Self(dir)
    }
    fn place(&self, known: Option<&'static Known>, any_case: bool) -> Place {
        Place {
            data: self.0.join("data"),
            game_dir: self.0.join("game"),
            known,
            any_case,
            appdata: None,
            my_games: None,
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A mod as if unpacked, listed last and on.
fn add_unpacked(place: &Place, id: &str, files: &[(&str, &str)]) {
    for (path, text) in files {
        write(&place.files().join(id).join(path), text);
    }
    place
        .change(|list| {
            list.mods.push(Entry {
                id: id.into(),
                name: id.into(),
                enabled: true,
                root: String::new(),
                source: String::new(),
                nexus: None,
                installer: false,
                chosen: Vec::new(),
            });
            Ok(())
        })
        .unwrap();
}

/// Adds an archive with no installer; its mod's id.
fn add_archive(place: &Place, archive: &Path) -> String {
    assert!(place.take_in(archive, None, None, None).unwrap().is_none());
    place.list().unwrap().mods.last().unwrap().id.clone()
}

fn read(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok()
}

#[test]
fn applies_and_goes_back_to_vanilla_exactly() {
    let scratch = Scratch::new("apply");
    let place = scratch.place(None, true);
    let game = &place.game_dir;
    write(&game.join("Game.exe"), "game");
    write(&game.join("Data/textures/sky.dds"), "vanilla sky");
    add_unpacked(
        &place,
        "loader",
        &[
            ("winhttp.dll", "loader"),
            ("BepInEx/core/BepInEx.dll", "core"),
        ],
    );
    add_unpacked(&place, "sky", &[("data/Textures/sky.dds", "first sky")]);
    add_unpacked(
        &place,
        "better-sky",
        &[("Data/textures/SKY.dds", "better sky")],
    );

    let overview = place.overview().unwrap();
    assert_eq!(overview.applied, Applied::No);
    assert_eq!(overview.mods[2].wins, vec![("sky".to_owned(), 1)]);
    assert_eq!(overview.mods[1].loses, vec![("better-sky".to_owned(), 1)]);

    place.apply().unwrap();
    assert_eq!(read(&game.join("winhttp.dll")).as_deref(), Some("loader"));
    // The game's own spelling, and the last mod's file.
    assert_eq!(
        read(&game.join("Data/textures/sky.dds")).as_deref(),
        Some("better sky")
    );
    assert!(!game.join("data").exists());
    assert_eq!(place.overview().unwrap().applied, Applied::Current(3));

    // The game writes a log and changes a mod's file while modded.
    write(&game.join("BepInEx/LogOutput.log"), "loaded");
    write(&game.join("winhttp.dll"), "changed by the game");

    place.restore().unwrap();
    assert_eq!(
        read(&game.join("Data/textures/sky.dds")).as_deref(),
        Some("vanilla sky")
    );
    assert!(!game.join("winhttp.dll").exists());
    assert!(!game.join("BepInEx").exists());
    assert_eq!(read(&game.join("Game.exe")).as_deref(), Some("game"));
    let overwrite = place.overwrite();
    assert_eq!(
        read(&overwrite.join("BepInEx/LogOutput.log")).as_deref(),
        Some("loaded")
    );
    assert_eq!(
        read(&overwrite.join("winhttp.dll")).as_deref(),
        Some("changed by the game")
    );
    assert_eq!(place.overview().unwrap().applied, Applied::No);
    assert_eq!(place.overview().unwrap().overwrite, 2);

    // Applied again, what the game wrote comes back after every mod.
    place.apply().unwrap();
    assert_eq!(
        read(&game.join("winhttp.dll")).as_deref(),
        Some("changed by the game")
    );
    assert_eq!(
        read(&game.join("BepInEx/LogOutput.log")).as_deref(),
        Some("loaded")
    );
    place.restore().unwrap();
    assert!(!game.join("BepInEx").exists());
}

fn date(path: &Path, time: SystemTime) {
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(time)
        .unwrap();
}

fn modified(path: &Path) -> SystemTime {
    fs::metadata(path).unwrap().modified().unwrap()
}

fn plugin_names(place: &Place) -> Vec<(String, bool)> {
    order_of(&place.overview().unwrap().plugins.unwrap().list)
}

/// A scratch Fallout 3 with a prefix: its three plugins dated as
/// Steam installs them (the master after its DLC), and two mods with a
/// plugin each, `Patch.esp` needing `Lib.esp`. The place, the game's
/// folder in `AppData/Local`, the prefix's user folder, `Data`, and
/// the time the game's plugins were dated from.
fn scratch_fallout(scratch: &Scratch) -> (Place, PathBuf, PathBuf, PathBuf, SystemTime) {
    use crate::plugins::tests::plugin;

    let fallout = KNOWN.iter().find(|known| known.nexus == "fallout3");
    let mut place = scratch.place(fallout, true);
    let user = scratch.0.join("pfx/drive_c/users/steamuser");
    fs::create_dir_all(&user).unwrap();
    let appdata = user.join("AppData/Local/Fallout3");
    place.appdata = Some(appdata.clone());
    let data = place.game_dir.join("Data");
    fs::create_dir_all(&data).unwrap();
    let installed = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    for (name, seconds) in FALLOUT_OWN.iter().zip(0..) {
        let masters: &[&str] = if *name == "Fallout3.esm" {
            &[]
        } else {
            &["Fallout3.esm"]
        };
        fs::write(data.join(name), plugin(1, masters)).unwrap();
        date(&data.join(name), installed + Duration::from_secs(seconds));
    }
    for (id, name, masters) in [
        ("lib", "Lib.esp", &["Fallout3.esm"][..]),
        ("patch", "Patch.esp", &["Fallout3.esm", "Lib.esp"][..]),
    ] {
        add_unpacked(&place, id, &[]);
        fs::create_dir_all(place.files().join(id)).unwrap();
        fs::write(place.files().join(id).join(name), plugin(0, masters)).unwrap();
    }
    (place, appdata, user, data, installed)
}

const FALLOUT_OWN: [&str; 3] = ["Anchorage.esm", "BrokenSteel.esm", "Fallout3.esm"];

/// The scratch Fallout 3's order, with these plugins on.
fn fallout_on(names: &[&str]) -> Vec<(String, bool)> {
    [
        "Fallout3.esm",
        "Anchorage.esm",
        "BrokenSteel.esm",
        "Lib.esp",
        "Patch.esp",
    ]
    .iter()
    .map(|name| ((*name).to_owned(), names.contains(name)))
    .collect()
}

#[test]
fn writes_the_plugin_order_on_apply_and_puts_the_game_back() {
    let scratch = Scratch::new("plugins");
    let (place, appdata, user, data, installed) = scratch_fallout(&scratch);
    let on = fallout_on;
    let own = FALLOUT_OWN;

    let plugins = place.overview().unwrap().plugins.unwrap();
    assert_eq!(plugins.file, Some(appdata.join("Plugins.txt")));
    let all = [
        "Fallout3.esm",
        "Anchorage.esm",
        "BrokenSteel.esm",
        "Lib.esp",
        "Patch.esp",
    ];
    assert_eq!(order_of(&plugins.list), on(&all));
    assert_eq!(plugins.list[4].from.as_deref(), Some("patch"));

    // Patch before its master shows; moved back, it does not.
    place
        .change_plugins(|plugins| plugins::moved(plugins, "Patch.esp", true))
        .unwrap();
    let list = place.overview().unwrap().plugins.unwrap().list;
    assert_eq!(list[3].after, ["Lib.esp"]);
    place
        .change_plugins(|plugins| plugins::moved(plugins, "Patch.esp", false))
        .unwrap();
    place
        .change_plugins(|plugins| {
            let mut order = order_of(plugins);
            order[1].1 = false;
            Some(order)
        })
        .unwrap();

    place.apply().unwrap();
    assert_eq!(
        read(&appdata.join("Plugins.txt")).as_deref(),
        Some("Fallout3.esm\r\nBrokenSteel.esm\r\nLib.esp\r\nPatch.esp\r\n")
    );
    let times: Vec<SystemTime> = all.iter().map(|name| modified(&data.join(name))).collect();
    assert!(times.is_sorted() && times[0] < times[1], "{times:?}");
    assert_eq!(place.overview().unwrap().applied, Applied::Current(2));
    // A move shows as a change; back again, it does not.
    place
        .change_plugins(|plugins| plugins::moved(plugins, "Patch.esp", true))
        .unwrap();
    assert_eq!(place.overview().unwrap().applied, Applied::Changed);
    place
        .change_plugins(|plugins| plugins::moved(plugins, "Patch.esp", false))
        .unwrap();
    assert_eq!(place.overview().unwrap().applied, Applied::Current(2));

    place.restore().unwrap();
    assert!(!user.join("AppData").exists(), "the folders Zendo made go");
    assert!(!data.join("Lib.esp").exists());
    for (name, seconds) in own.iter().zip(0..) {
        assert_eq!(
            modified(&data.join(name)),
            installed + Duration::from_secs(seconds)
        );
    }
}

#[test]
fn the_games_own_list_decides_first_and_comes_back() {
    let scratch = Scratch::new("plugins-own");
    let (place, appdata, ..) = scratch_fallout(&scratch);
    let on = fallout_on;
    // A list the game had decides its own plugins' first state, keeps
    // its spelling (the launcher writes `plugins.txt`), and comes back
    // as it was.
    let original = "Fallout3.esm\r\nBrokenSteel.esm\r\n";
    write(&appdata.join("plugins.txt"), original);
    place
        .change(|list| {
            list.plugins.clear();
            Ok(())
        })
        .unwrap();
    assert_eq!(
        plugin_names(&place),
        on(&["Fallout3.esm", "BrokenSteel.esm", "Lib.esp", "Patch.esp"])
    );
    place.apply().unwrap();
    assert_ne!(
        read(&appdata.join("plugins.txt")).as_deref(),
        Some(original)
    );
    let names: Vec<String> = fs::read_dir(&appdata)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["plugins.txt"], "no second list beside it");
    // Applied, the list in the prefix is Zendo's; the game's still counts.
    assert_eq!(
        plugin_names(&place),
        on(&["Fallout3.esm", "BrokenSteel.esm", "Lib.esp", "Patch.esp"])
    );
    place.restore().unwrap();
    assert_eq!(
        read(&appdata.join("plugins.txt")).as_deref(),
        Some(original)
    );
}

#[test]
fn each_profile_keeps_its_plugin_order() {
    let entry = |name: &str, enabled| PluginEntry {
        name: name.into(),
        enabled,
    };
    let mut list = List {
        plugins: vec![entry("A.esp", true), entry("B.esp", true)],
        ..List::default()
    };
    list.new_profile("Other").unwrap();
    list.keep_plugins(&[("B.esp".into(), false), ("A.esp".into(), true)]);
    assert_eq!(list.plugins, [entry("B.esp", false), entry("A.esp", true)]);
    list.switch_profile(DEFAULT_PROFILE).unwrap();
    assert_eq!(list.plugins, [entry("A.esp", true), entry("B.esp", true)]);
    // Kept, a plugin not there now stays after the order given.
    list.keep_plugins(&[("B.esp".into(), true)]);
    assert_eq!(list.plugins, [entry("B.esp", true), entry("A.esp", true)]);
}

#[test]
fn a_linux_game_keeps_case_apart_and_a_change_shows() {
    let scratch = Scratch::new("case");
    let place = scratch.place(None, false);
    write(&place.game_dir.join("Mods/readme.txt"), "game's");
    add_unpacked(&place, "a", &[("mods/A/manifest.json", "{}")]);
    place.apply().unwrap();
    assert!(place.game_dir.join("mods/A/manifest.json").is_file());
    assert!(place.game_dir.join("Mods/readme.txt").is_file());
    place
        .change(|list| {
            list.mods[0].enabled = false;
            Ok(())
        })
        .unwrap();
    assert_eq!(place.overview().unwrap().applied, Applied::Changed);
    place.apply().unwrap();
    assert!(!place.game_dir.join("mods").exists());
    assert_eq!(place.overview().unwrap().applied, Applied::No);
}

#[test]
fn a_failed_apply_takes_out_what_went_in() {
    let scratch = Scratch::new("fail");
    let place = scratch.place(None, false);
    // A file where the mod needs a folder.
    write(&place.game_dir.join("plugins"), "a file");
    add_unpacked(&place, "a", &[("aaa.txt", "a"), ("plugins/x.dll", "x")]);
    let err = place.apply().unwrap_err();
    assert!(format!("{err:#}").contains("needs a folder"), "{err:#}");
    assert!(!place.game_dir.join("aaa.txt").exists());
    assert!(place.manifest().unwrap().is_none());
}

#[test]
fn finds_the_mod_inside_its_wrappers() {
    let scratch = Scratch::new("root");
    let unpacked = scratch.0.join("unpacked");
    let cyberpunk = KNOWN.iter().find(|known| known.nexus == "cyberpunk2077");
    write(
        &unpacked.join("Cool Mod 1.2/Cool Mod/archive/pc/mod/cool.archive"),
        "x",
    );
    write(&unpacked.join("Cool Mod 1.2/readme.txt"), "x");
    let place = scratch.place(cyberpunk, true);
    assert_eq!(place.find_root(&unpacked), "Cool Mod 1.2/Cool Mod");

    let skyrim = KNOWN
        .iter()
        .find(|known| known.nexus == "skyrimspecialedition");
    let unpacked = scratch.0.join("skyrim");
    write(&unpacked.join("data/meshes/a.nif"), "x");
    write(&unpacked.join("fomod/info.xml"), "x");
    assert_eq!(scratch.place(skyrim, true).find_root(&unpacked), "data");
    // Stardew mods are folders of their own, kept.
    let stardew = KNOWN.iter().find(|known| known.nexus == "stardewvalley");
    let unpacked = scratch.0.join("stardew");
    write(&unpacked.join("ContentPatcher/manifest.json"), "{}");
    assert_eq!(scratch.place(stardew, false).find_root(&unpacked), "");
    assert_eq!(scratch.place(None, false).find_root(&unpacked), "");
}

#[test]
fn unpacks_an_archive_and_refuses_links() {
    if launch::which("7z").is_none() {
        return;
    }
    let scratch = Scratch::new("unpack");
    let place = scratch.place(None, true);
    let source = scratch.0.join("source");
    write(&source.join("BepInEx/core/BepInEx.dll"), "core");
    write(&source.join("winhttp.dll"), "loader");
    let archive = scratch.0.join("BepInEx_win_x64_5.4.23.2.zip");
    let status = Command::new("7z")
        .args(["a", "-bso0", "-bsp0"])
        .arg(&archive)
        .arg(".")
        .current_dir(&source)
        .status()
        .unwrap();
    assert!(status.success());
    let id = add_archive(&place, &archive);
    assert_eq!(id, "bepinex-win-x64-5-4-23-2");
    let overview = place.overview().unwrap();
    assert_eq!(overview.mods[0].name, "BepInEx_win_x64_5.4.23.2");
    assert_eq!(overview.mods[0].files, 2);

    std::os::unix::fs::symlink("/etc/passwd", source.join("passwd")).unwrap();
    let linked = scratch.0.join("linked.tar");
    let status = Command::new("bsdtar")
        .arg("-cf")
        .arg(&linked)
        .arg("-C")
        .arg(&source)
        .arg(".")
        .status()
        .unwrap();
    assert!(status.success());
    let err = place.take_in(&linked, None, None, None).unwrap_err();
    assert!(format!("{err:#}").contains("links"), "{err:#}");
    assert_eq!(place.overview().unwrap().mods.len(), 1);
    assert_eq!(entries(&place.files()).0, vec![id]);
}

fn on(overview: &Overview) -> Vec<&str> {
    overview
        .mods
        .iter()
        .map(|info| if info.enabled { info.id.as_str() } else { "-" })
        .collect()
}

#[test]
fn profiles_keep_their_own_order_and_mods_on() {
    let scratch = Scratch::new("profiles");
    let place = scratch.place(None, false);
    add_unpacked(&place, "a", &[("a.txt", "a")]);
    add_unpacked(&place, "b", &[("b.txt", "b")]);
    let overview = place.overview().unwrap();
    assert_eq!(overview.profile, DEFAULT_PROFILE);
    assert_eq!(overview.profiles, [DEFAULT_PROFILE]);

    let change = |change: ProfileChange| place.change_profiles(&change, true);
    change(ProfileChange::New("Light".into())).unwrap();
    let overview = place.overview().unwrap();
    assert_eq!(overview.profile, "Light");
    assert_eq!(overview.profiles, ["Default", "Light"]);
    assert_eq!(on(&overview), ["a", "b"], "a copy of the one in use");

    // In Light: b first, a off.
    place
        .change(|list| {
            list.mods.reverse();
            list.mods[1].enabled = false;
            Ok(())
        })
        .unwrap();
    add_unpacked(&place, "c", &[("c.txt", "c")]);
    assert_eq!(on(&place.overview().unwrap()), ["b", "-", "c"]);

    change(ProfileChange::Switch("Default".into())).unwrap();
    let overview = place.overview().unwrap();
    let ids: Vec<&str> = overview.mods.iter().map(|info| info.id.as_str()).collect();
    assert_eq!(ids, ["a", "b", "c"]);
    assert_eq!(on(&overview), ["a", "b", "-"], "c came after: last and off");

    change(ProfileChange::Switch("Light".into())).unwrap();
    assert_eq!(on(&place.overview().unwrap()), ["b", "-", "c"]);

    for (bad, why) in [
        ("  ", "name"),
        ("default", "already"),
        (&"x".repeat(41), "40"),
        ("two\nlines", "one line"),
    ] {
        let err = change(ProfileChange::New(bad.to_owned())).unwrap_err();
        assert!(err.to_string().contains(why), "{bad:?}: {err}");
    }
    change(ProfileChange::Rename {
        old: "Light".into(),
        new: "light".into(),
    })
    .unwrap();
    change(ProfileChange::Rename {
        old: "Default".into(),
        new: "Everything".into(),
    })
    .unwrap();
    assert_eq!(place.overview().unwrap().profiles, ["Everything", "light"]);

    // A mod deleted leaves every profile.
    place
        .change(|list| {
            list.mods.retain(|entry| entry.id != "c");
            Ok(())
        })
        .unwrap();
    change(ProfileChange::Switch("Everything".into())).unwrap();
    assert_eq!(on(&place.overview().unwrap()), ["a", "b"]);
    // The one in use goes after a switch to the next; the last stays.
    change(ProfileChange::Delete("Everything".into())).unwrap();
    let overview = place.overview().unwrap();
    assert_eq!(
        (overview.profile.as_str(), on(&overview)),
        ("light", vec!["b", "-"])
    );
    assert_eq!(overview.profiles, ["light"]);
    let err = change(ProfileChange::Delete("light".into())).unwrap_err();
    assert!(err.to_string().contains("at least one"), "{err}");
    change(ProfileChange::New("Everything".into())).unwrap();

    // Written and read back.
    let text = fs::read_to_string(place.list_path()).unwrap();
    assert!(text.contains("active_profile = \"Everything\""), "{text}");
}

#[test]
fn switching_while_applied_applies_the_other_profile() {
    let scratch = Scratch::new("profile-apply");
    let place = scratch.place(None, false);
    add_unpacked(&place, "a", &[("same.txt", "from a")]);
    add_unpacked(&place, "b", &[("same.txt", "from b")]);
    place
        .change_profiles(&ProfileChange::New("Only a".into()), true)
        .unwrap();
    place
        .change(|list| {
            list.mods[1].enabled = false;
            Ok(())
        })
        .unwrap();
    place.apply().unwrap();
    let file = place.game_dir.join("same.txt");
    assert_eq!(read(&file).as_deref(), Some("from a"));

    place
        .change_profiles(&ProfileChange::Switch("Default".into()), true)
        .unwrap();
    assert_eq!(read(&file).as_deref(), Some("from b"));
    assert_eq!(place.overview().unwrap().applied, Applied::Current(2));

    // With the game running the list changes and the game waits.
    place
        .change_profiles(&ProfileChange::Switch("Only a".into()), false)
        .unwrap();
    assert_eq!(read(&file).as_deref(), Some("from b"));
    assert_eq!(place.overview().unwrap().applied, Applied::Changed);

    // Not applied: switching applies nothing.
    place.restore().unwrap();
    place
        .change_profiles(&ProfileChange::Switch("Default".into()), true)
        .unwrap();
    assert_eq!(read(&file), None);
}

/// A tar of `files` (path, text) in a folder made for it.
fn tar(scratch: &Scratch, name: &str, files: &[(&str, &str)]) -> PathBuf {
    let source = scratch.0.join(format!("source-{name}"));
    for (path, text) in files {
        write(&source.join(path), text);
    }
    let archive = scratch.0.join(format!("{name}.tar"));
    let status = Command::new("bsdtar")
        .arg("-cf")
        .arg(&archive)
        .arg("-C")
        .arg(&source)
        .arg(".")
        .status()
        .unwrap();
    assert!(status.success());
    archive
}

const INSTALLER: &str = r#"<config>
  <moduleName>Body Mod</moduleName>
  <requiredInstallFiles><folder source="core" destination=""/></requiredInstallFiles>
  <installSteps><installStep name="Body"><optionalFileGroups>
    <group name="Pick" type="SelectExactlyOne"><plugins order="Explicit">
      <plugin name="Slim"><files><folder source="Slim" destination="Meshes"/></files>
        <typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
      <plugin name="Strong"><files><folder source="Strong" destination="meshes"/></files>
        <typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
    </plugins></group>
  </optionalFileGroups></installStep></installSteps>
</config>"#;

#[test]
fn an_installer_builds_the_mod_from_the_options_chosen() {
    if launch::which("bsdtar").is_none() {
        return;
    }
    let scratch = Scratch::new("fomod");
    let place = scratch.place(None, true);
    write(&place.game_dir.join("Meshes/game.nif"), "game");
    let files = [
        ("Body Mod 1.0/FOMOD/moduleconfig.xml", INSTALLER),
        ("Body Mod 1.0/Core/body.esp", "plugin"),
        ("Body Mod 1.0/slim/body.nif", "slim"),
        ("Body Mod 1.0/Strong/body.nif", "strong"),
    ];
    let archive = tar(&scratch, "body", &files);
    let pending = place.take_in(&archive, None, None, None).unwrap().unwrap();
    assert_eq!(pending.installer.name, "Body Mod");
    assert!(pending.present.files.contains("meshes/game.nif"));
    assert!(!pending.replaces());
    assert!(
        place.list().unwrap().mods.is_empty(),
        "not in until installed"
    );

    // Cancel leaves nothing.
    cancel_install(&pending);
    assert_eq!(entries(&place.files()), (Vec::new(), Vec::new()));

    let pending = place.take_in(&archive, None, None, None).unwrap().unwrap();
    let mut selection = pending.installer.default_selection(&pending.present);
    assert_eq!(selection.group(0, 0), [true, false], "the first must be on");
    place.install(&pending, &selection).unwrap();
    let id = place.list().unwrap().mods[0].id.clone();
    let built = place.files().join(&id);
    assert_eq!(read(&built.join("body.esp")).as_deref(), Some("plugin"));
    assert_eq!(
        read(&built.join("Meshes/body.nif")).as_deref(),
        Some("slim")
    );
    assert!(find_installer(&place.installers().join(&id)).is_some());
    let overview = place.overview().unwrap();
    assert!(overview.mods[0].installer);
    assert_eq!(overview.mods[0].files, 2);
    place.apply().unwrap();
    assert_eq!(
        read(&place.game_dir.join("Meshes/body.nif")).as_deref(),
        Some("slim")
    );

    // Options…: Strong now, whose folder the installer spells
    // `meshes`; Apply puts it in the game's `Meshes`, a Windows game.
    let again = reinstall_at(&place, &id);
    assert!(again.replaces());
    let mut chosen = again
        .installer
        .selection_from(&again.chosen, &again.present);
    assert_eq!(chosen.group(0, 0), [true, false], "last time's options");
    again
        .installer
        .toggle(&mut chosen, 0, 0, 1, true, &again.present);
    place.install(&again, &chosen).unwrap();
    assert_eq!(
        read(&built.join("meshes/body.nif")).as_deref(),
        Some("strong")
    );
    assert_eq!(entries(&built).0, ["meshes"], "the slim files are gone");
    assert_eq!(place.overview().unwrap().applied, Applied::Changed);
    place.apply().unwrap();
    assert_eq!(
        read(&place.game_dir.join("Meshes/body.nif")).as_deref(),
        Some("strong")
    );
    assert!(place.installers().join(&id).is_dir(), "kept for next time");

    // An installer that climbs out of the mod is refused.
    selection = again.installer.default_selection(&again.present);
    let evil = INSTALLER.replace(r#"destination="Meshes""#, r#"destination="..\..\evil""#);
    let evil_archive = tar(
        &scratch,
        "evil",
        &[
            ("fomod/ModuleConfig.xml", evil.as_str()),
            ("Slim/body.nif", "x"),
            ("core/a.esp", "a"),
        ],
    );
    let pending = place
        .take_in(&evil_archive, None, None, None)
        .unwrap()
        .unwrap();
    let err = place.install(&pending, &selection).unwrap_err();
    assert!(err.to_string().contains("outside"), "{err}");
    assert!(!scratch.0.join("evil").exists());
    cancel_install(&pending);
    assert_eq!(place.list().unwrap().mods.len(), 1);
    place.restore().unwrap();
    assert_eq!(
        read(&place.game_dir.join("Meshes/game.nif")).as_deref(),
        Some("game")
    );
    assert_eq!(entries(&place.game_dir.join("Meshes")).1, ["game.nif"]);
}

/// A real archive with a FOMOD installer, installed with its default
/// options and every checkbox on, as in `ZENDO_TEST_FOMOD_ARCHIVE=<archive>
/// cargo test -p zendo-core real_installer -- --ignored --nocapture`.
#[test]
#[ignore = "needs an archive with a FOMOD installer in ZENDO_TEST_FOMOD_ARCHIVE"]
fn a_real_installer_archive_builds() {
    let archive = std::env::var("ZENDO_TEST_FOMOD_ARCHIVE").expect("ZENDO_TEST_FOMOD_ARCHIVE");
    let scratch = Scratch::new("real-fomod");
    let place = scratch.place(None, true);
    let pending = place
        .take_in(Path::new(&archive), None, None, None)
        .unwrap()
        .expect("an installer");
    let installer = &pending.installer;
    let mut selection = installer.default_selection(&pending.present);
    for at in installer.visible_steps(&selection, &pending.present) {
        for (g, group) in installer.steps[at].groups.iter().enumerate() {
            if !group.kind.single() {
                for o in 0..group.options.len() {
                    installer.toggle(&mut selection, at, g, o, true, &pending.present);
                }
            }
        }
    }
    println!(
        "{:?}: {:?}",
        installer.name,
        installer.chosen(&selection, &pending.present)
    );
    place.install(&pending, &selection).unwrap();
    let id = place.list().unwrap().mods[0].id.clone();
    for file in walk(&place.files().join(&id)).unwrap() {
        println!("  {}", file.display());
    }
    place.apply().unwrap();
    place.restore().unwrap();
    assert_eq!(entries(&place.game_dir), (Vec::new(), Vec::new()));
}

/// [`reinstall`] on a scratch place.
fn reinstall_at(place: &Place, id: &str) -> Pending {
    let list = place.list().unwrap();
    let entry = list.mods.iter().find(|entry| entry.id == id).unwrap();
    let root = find_installer(&place.installers().join(id)).unwrap();
    Pending {
        name: entry.name.clone(),
        installer: read_installer(&root).unwrap(),
        present: place.present(&list),
        chosen: entry.chosen.clone(),
        root,
        what: PendingKind::Options { id: id.to_owned() },
    }
}

#[test]
fn a_nexus_update_takes_the_mods_place_and_shows_as_a_change() {
    if launch::which("bsdtar").is_none() {
        return;
    }
    let scratch = Scratch::new("nexus");
    let place = scratch.place(None, false);
    add_unpacked(&place, "first", &[("first.txt", "first")]);
    let files: Files = serde_json::from_str(
        r#"{"files":[{"file_id":10,"version":"1.0","file_name":"m.tar"},
                     {"file_id":11,"version":"1.1","file_name":"m.tar"},
                     {"file_id":20,"version":"1.0","file_name":"extra.tar"}],
            "file_updates":[{"old_file_id":10,"new_file_id":11}]}"#,
    )
    .unwrap();
    let fetched = |file_id: u64, text: &str| {
        let source = scratch.0.join(format!("source-{file_id}"));
        write(&source.join("mod.txt"), text);
        let archive = scratch.0.join(format!("m-{file_id}.tar"));
        let status = Command::new("bsdtar")
            .arg("-cf")
            .arg(&archive)
            .arg("-C")
            .arg(&source)
            .arg(".")
            .status()
            .unwrap();
        assert!(status.success());
        nexus::Fetched {
            archive,
            mod_name: "Cool Mod".into(),
            file: files.get(file_id).unwrap().clone(),
            files: files.clone(),
        }
    };

    let old = fetched(10, "old");
    assert!(!place.add_fetched(&old, "catquest", 7).unwrap().1);
    assert!(
        !old.archive.exists(),
        "the download is deleted once unpacked"
    );
    add_unpacked(&place, "last", &[("last.txt", "last")]);
    place.apply().unwrap();
    let game = place.game_dir.clone();
    assert_eq!(read(&game.join("mod.txt")).as_deref(), Some("old"));
    assert_eq!(place.overview().unwrap().applied, Applied::Current(3));

    assert!(
        place
            .add_fetched(&fetched(11, "new"), "catquest", 7)
            .unwrap()
            .1
    );
    let overview = place.overview().unwrap();
    let names: Vec<&str> = overview
        .mods
        .iter()
        .map(|info| info.name.as_str())
        .collect();
    assert_eq!(names, ["first", "Cool Mod", "last"], "kept in its place");
    let nexus = overview.mods[1].nexus.as_ref().unwrap();
    assert_eq!((nexus.file_id, nexus.version.as_str()), (11, "1.1"));
    assert_eq!(overview.applied, Applied::Changed);
    assert_eq!(entries(&place.files()).0.len(), 3, "nothing left aside");
    place.apply().unwrap();
    assert_eq!(read(&game.join("mod.txt")).as_deref(), Some("new"));

    // Another file of the same mod is a mod of its own.
    assert!(
        !place
            .add_fetched(&fetched(20, "extra"), "catquest", 7)
            .unwrap()
            .1
    );
    assert_eq!(place.overview().unwrap().mods.len(), 4);
    place.restore().unwrap();
    assert_eq!(entries(&game), (Vec::new(), Vec::new()));
}

/// A real mod archive, added, applied to a game folder made for the
/// test and taken out again, as in `ZENDO_TEST_ARCHIVE=<archive> cargo
/// test -p zendo-core mods -- --ignored`.
#[test]
#[ignore = "needs a real mod archive in ZENDO_TEST_ARCHIVE"]
fn a_real_archive_goes_in_and_out() {
    let Some(archive) = std::env::var_os("ZENDO_TEST_ARCHIVE") else {
        return;
    };
    let scratch = Scratch::new("real");
    let place = scratch.place(None, true);
    write(&place.game_dir.join("Game.exe"), "game");
    write(&place.game_dir.join("Game_Data/level0"), "level");
    add_archive(&place, Path::new(&archive));
    let overview = place.overview().unwrap();
    assert!(overview.mods[0].files > 0);
    place.apply().unwrap();
    let placed = place.manifest().unwrap().unwrap();
    assert_eq!(placed.files.len(), overview.mods[0].files);
    place.restore().unwrap();
    let mut left = walk(&place.game_dir).unwrap();
    left.sort();
    assert_eq!(
        left,
        vec![PathBuf::from("Game.exe"), PathBuf::from("Game_Data/level0")]
    );
}

#[test]
fn knows_games_by_steam_id_or_title() {
    let game = |id: &str, title: &str, source| {
        Game::new(
            id.into(),
            title.into(),
            source,
            crate::Launch::Uri { uri: String::new() },
        )
    };
    let stardew = game("steam:413150", "Stardew Valley", Source::Steam);
    assert_eq!(known(&stardew).map(|known| known.folder), Some("Mods"));
    let gog = game("gog:1", "Cyberpunk 2077", Source::Gog);
    assert_eq!(known(&gog).map(|known| known.nexus), Some("cyberpunk2077"));
    let unknown = game("steam:1", "Stardew Valley", Source::Steam);
    assert_eq!(known(&unknown), None);
    let goty = game(
        "steam:22370",
        "Fallout 3 - Game of the Year Edition",
        Source::Steam,
    );
    assert_eq!(known(&goty).map(|known| known.nexus), Some("fallout3"));
    assert_eq!(known(&goty).map(|known| known.folder), Some("Data"));
}

#[test]
fn names_mods_from_their_archives() {
    assert_eq!(
        archive_name(Path::new("/d/SkyUI_5_2_SE-12604-5-2SE.7z")),
        "SkyUI_5_2_SE-12604-5-2SE"
    );
    assert_eq!(archive_name(Path::new("/d/mod.tar.gz")), "mod");
    assert!(check_relative(Path::new("BepInEx/plugins")).is_ok());
    assert!(check_relative(Path::new("../x")).is_err());
    assert!(check_relative(Path::new("/x")).is_err());
}
