//! FOMOD installers: the `fomod/ModuleConfig.xml` many mods for Bethesda's
//! games (and some others) carry, which asks the user to choose options in
//! steps and says which of the archive's files each option installs.
//!
//! This reads the file (UTF-8 or UTF-16, element names in any case),
//! orders steps, groups and options as it says (`Ascending`, the default,
//! `Descending` or `Explicit`), and works out from a selection the steps
//! shown, each option's type (required, recommended, not usable…) and the
//! files to install, conditional ones included. Building the mod from those
//! files is `mods.rs`'.
//!
//! A file condition (`fileDependency`) sees a file as active when the game's
//! mod folder or a mod that is on has it, inactive when a plugin that is
//! off is there instead, and missing otherwise. Game and FOMM version
//! conditions always hold.

use std::collections::{HashMap, HashSet};

use anyhow::{Context as _, bail};
use roxmltree::{Document, Node, ParsingOptions};

/// An installer, as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installer {
    pub name: String,
    /// The mod's picture, a path in the archive.
    pub image: Option<String>,
    /// What the mod says it needs.
    pub needs: Option<Condition>,
    /// Installed whatever is chosen.
    pub required: Vec<Install>,
    pub steps: Vec<Step>,
    /// Installed when their condition holds once everything is chosen.
    pub conditional: Vec<(Condition, Vec<Install>)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub name: String,
    /// Shown only when this holds, with the flags set by the steps before.
    pub visible: Option<Condition>,
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub name: String,
    pub kind: GroupKind,
    pub options: Vec<Choice>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupKind {
    AtLeastOne,
    AtMostOne,
    ExactlyOne,
    All,
    Any,
}

impl GroupKind {
    /// One option at most: shown as a radio.
    #[must_use]
    pub fn single(self) -> bool {
        matches!(self, Self::AtMostOne | Self::ExactlyOne)
    }
}

/// One option ("plugin" in the file).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub name: String,
    pub description: String,
    pub image: Option<String>,
    pub files: Vec<Install>,
    /// Flags it sets when chosen.
    pub flags: Vec<(String, String)>,
    pub kind: Kind,
}

/// An option's type, fixed or depending on what is chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kind {
    pub default: OptionType,
    pub patterns: Vec<(Condition, OptionType)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionType {
    /// Always on.
    Required,
    Recommended,
    Optional,
    CouldBeUsable,
    /// Never on.
    NotUsable,
}

/// A file or folder of the archive and where it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Install {
    /// From the folder holding `fomod/`, with `/` between parts.
    pub source: String,
    /// From the mod's folder; `None` for the source's own path.
    pub destination: Option<String>,
    /// For a file: the destination was empty or ended in a separator, so
    /// it is a folder the file goes into under its own name.
    pub into_folder: bool,
    pub folder: bool,
    /// A higher one wins a file both install.
    pub priority: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    All(Vec<Condition>),
    Any(Vec<Condition>),
    Flag {
        name: String,
        value: String,
    },
    File {
        file: String,
        state: FileState,
    },
    /// Game and FOMM versions, not checked.
    Holds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileState {
    Active,
    Inactive,
    Missing,
}

/// What file conditions look at: the files in the game's mod folder and in
/// the mods that are on, and of those the plugins that are off (a Bethesda
/// game's), lowercase, from the mod folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Present {
    pub files: HashSet<String>,
    pub inactive: HashSet<String>,
}

impl Present {
    fn has(&self, file: &str) -> bool {
        self.files.contains(&normalise(file).to_lowercase())
    }

    fn off(&self, file: &str) -> bool {
        self.inactive.contains(&normalise(file).to_lowercase())
    }
}

/// What is chosen: for each step, group and option, whether it is on.
/// Only an installer makes one ([`Installer::default_selection`],
/// [`Installer::selection_from`]), so it always has that installer's shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection(Vec<Vec<Vec<bool>>>);

impl Selection {
    /// Whether an option is on; `false` for one the installer lacks.
    #[must_use]
    pub fn is_on(&self, step: usize, group: usize, option: usize) -> bool {
        self.group(step, group)
            .get(option)
            .copied()
            .unwrap_or(false)
    }

    /// Each option of a group, on or not; empty for a group it lacks.
    #[must_use]
    pub fn group(&self, step: usize, group: usize) -> &[bool] {
        self.step(step).get(group).map_or(&[], Vec::as_slice)
    }

    /// Each group of a step; empty for a step it lacks.
    #[must_use]
    pub fn step(&self, step: usize) -> &[Vec<bool>] {
        self.0.get(step).map_or(&[], Vec::as_slice)
    }
}

impl Condition {
    #[must_use]
    pub fn holds(&self, flags: &HashMap<String, String>, present: &Present) -> bool {
        match self {
            Self::All(all) => all.iter().all(|one| one.holds(flags, present)),
            Self::Any(any) => any.iter().any(|one| one.holds(flags, present)),
            // An unset flag reads as empty.
            Self::Flag { name, value } => flags.get(name).map_or("", String::as_str) == value,
            // A file there is active unless it is a plugin that is off.
            Self::File { file, state } => match state {
                FileState::Active => present.has(file) && !present.off(file),
                FileState::Inactive => present.has(file) && present.off(file),
                FileState::Missing => !present.has(file),
            },
            Self::Holds => true,
        }
    }
}

impl Kind {
    #[must_use]
    pub fn at(&self, flags: &HashMap<String, String>, present: &Present) -> OptionType {
        self.patterns
            .iter()
            .find(|(condition, _)| condition.holds(flags, present))
            .map_or(self.default, |(_, kind)| *kind)
    }
}

impl Installer {
    /// Reads a `ModuleConfig.xml`.
    ///
    /// # Errors
    ///
    /// When it is not XML or not an installer.
    pub fn parse(bytes: &[u8]) -> anyhow::Result<Self> {
        let xml = decode(bytes);
        let options = ParsingOptions {
            allow_dtd: true,
            ..ParsingOptions::default()
        };
        let document =
            Document::parse_with_options(&xml, options).context("ModuleConfig.xml is not XML")?;
        let config = document.root_element();
        if !named(config, "config") {
            bail!("ModuleConfig.xml is not a FOMOD installer");
        }
        let mut steps: Vec<Step> = child(config, "installSteps")
            .map(|steps| {
                let mut list: Vec<Step> = children(steps, "installStep").map(step).collect();
                sort(&mut list, order(steps), |step| &step.name);
                list
            })
            .unwrap_or_default();
        steps.retain(|step| !step.groups.is_empty());
        let conditional = child(config, "conditionalFileInstalls")
            .and_then(|node| child(node, "patterns"))
            .map(|patterns| {
                children(patterns, "pattern")
                    .map(|pattern| {
                        (
                            child(pattern, "dependencies").map_or(Condition::Holds, condition),
                            child(pattern, "files").map(installs).unwrap_or_default(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(Self {
            name: child(config, "moduleName")
                .map(text)
                .filter(|name| !name.is_empty())
                .unwrap_or_default(),
            image: child(config, "moduleImage").and_then(|node| path_attribute(node, "path")),
            needs: child(config, "moduleDependencies").map(condition),
            required: child(config, "requiredInstallFiles")
                .map(installs)
                .unwrap_or_default(),
            steps,
            conditional,
        })
    }

    /// The selection the installer starts with: required and recommended
    /// options on, and the first usable one where one must be.
    #[must_use]
    pub fn default_selection(&self, present: &Present) -> Selection {
        let mut selection = Selection(
            self.steps
                .iter()
                .map(|step| {
                    step.groups
                        .iter()
                        .map(|group| vec![false; group.options.len()])
                        .collect()
                })
                .collect(),
        );
        for at in 0..self.steps.len() {
            for (g, group) in self.steps[at].groups.iter().enumerate() {
                // The steps before, and the groups before in this one.
                let mut flags = self.flags_before(&selection, at, present);
                self.set_flags(&selection, at, &mut flags);
                let types: Vec<OptionType> = group
                    .options
                    .iter()
                    .map(|option| option.kind.at(&flags, present))
                    .collect();
                let picks = &mut selection.0[at][g];
                for (o, kind) in types.iter().enumerate() {
                    picks[o] = match group.kind {
                        GroupKind::All => *kind != OptionType::NotUsable,
                        _ => matches!(kind, OptionType::Required | OptionType::Recommended),
                    };
                }
                settle(group.kind, picks, &types, None);
            }
            self.settle_step(&mut selection, at, present);
        }
        selection
    }

    /// Settles every group of a step with its options' types now: an
    /// option another choice made required comes on.
    fn settle_step(&self, selection: &mut Selection, at: usize, present: &Present) {
        let types = self.types(selection, at, present);
        for (g, group) in self.steps[at].groups.iter().enumerate() {
            settle(group.kind, &mut selection.0[at][g], &types[g], None);
        }
    }

    /// Each option's type in a step, with the flags the steps before and
    /// the options on in this one set.
    #[must_use]
    pub fn types(
        &self,
        selection: &Selection,
        at: usize,
        present: &Present,
    ) -> Vec<Vec<OptionType>> {
        let Some(step) = self.steps.get(at) else {
            return Vec::new();
        };
        let mut flags = self.flags_before(selection, at, present);
        self.set_flags(selection, at, &mut flags);
        step.groups
            .iter()
            .map(|group| {
                group
                    .options
                    .iter()
                    .map(|option| option.kind.at(&flags, present))
                    .collect()
            })
            .collect()
    }

    /// Turns an option on or off, keeping its group's rule (one at most,
    /// at least one…) and required and unusable options as they must be.
    /// An option the installer lacks is left alone.
    pub fn toggle(
        &self,
        selection: &mut Selection,
        at: usize,
        group: usize,
        option: usize,
        on: bool,
        present: &Present,
    ) {
        let types = self.types(selection, at, present);
        let (Some(kind), Some(types), Some(picks)) = (
            self.steps
                .get(at)
                .and_then(|step| step.groups.get(group))
                .map(|group| group.kind),
            types.get(group),
            selection.0.get_mut(at).and_then(|step| step.get_mut(group)),
        ) else {
            return;
        };
        match types.get(option) {
            None | Some(OptionType::Required | OptionType::NotUsable) => return,
            Some(_) => {}
        }
        if kind.single() && types.contains(&OptionType::Required) {
            return;
        }
        if kind == GroupKind::All {
            return;
        }
        if kind.single() && on {
            picks.fill(false);
        }
        picks[option] = on;
        settle(kind, picks, types, Some(option));
        self.settle_step(selection, at, present);
    }

    /// The steps shown, in order: each one's condition with the flags the
    /// shown steps before it set.
    #[must_use]
    pub fn visible_steps(&self, selection: &Selection, present: &Present) -> Vec<usize> {
        let mut flags = HashMap::new();
        let mut shown = Vec::new();
        for (at, step) in self.steps.iter().enumerate() {
            if step
                .visible
                .as_ref()
                .is_none_or(|condition| condition.holds(&flags, present))
            {
                shown.push(at);
                self.set_flags(selection, at, &mut flags);
            }
        }
        shown
    }

    /// The flags set by the steps shown before `at`.
    fn flags_before(
        &self,
        selection: &Selection,
        at: usize,
        present: &Present,
    ) -> HashMap<String, String> {
        let mut flags = HashMap::new();
        for (before, step) in self.steps.iter().enumerate().take(at) {
            if step
                .visible
                .as_ref()
                .is_none_or(|condition| condition.holds(&flags, present))
            {
                self.set_flags(selection, before, &mut flags);
            }
        }
        flags
    }

    fn set_flags(&self, selection: &Selection, at: usize, flags: &mut HashMap<String, String>) {
        for (g, group) in self.steps[at].groups.iter().enumerate() {
            for (o, option) in group.options.iter().enumerate() {
                if selection.is_on(at, g, o) {
                    for (name, value) in &option.flags {
                        flags.insert(name.clone(), value.clone());
                    }
                }
            }
        }
    }

    /// What to install for a selection, lowest priority first so a higher
    /// one is placed last and wins.
    #[must_use]
    pub fn files(&self, selection: &Selection, present: &Present) -> Vec<Install> {
        let mut flags = HashMap::new();
        let mut files: Vec<Install> = self.required.clone();
        for at in self.visible_steps(selection, present) {
            let types = self.types(selection, at, present);
            for (g, group) in self.steps[at].groups.iter().enumerate() {
                for (o, option) in group.options.iter().enumerate() {
                    if selection.is_on(at, g, o) && types[g][o] != OptionType::NotUsable {
                        files.extend(option.files.iter().cloned());
                    }
                }
            }
            self.set_flags(selection, at, &mut flags);
        }
        for (condition, installs) in &self.conditional {
            if condition.holds(&flags, present) {
                files.extend(installs.iter().cloned());
            }
        }
        files.sort_by_key(|install| install.priority);
        files
    }

    /// The options on in the steps shown, as `step/group/option`, to choose
    /// them again next time.
    #[must_use]
    pub fn chosen(&self, selection: &Selection, present: &Present) -> Vec<String> {
        let mut chosen = Vec::new();
        for at in self.visible_steps(selection, present) {
            let step = &self.steps[at];
            for (g, group) in step.groups.iter().enumerate() {
                for (o, option) in group.options.iter().enumerate() {
                    if selection.is_on(at, g, o) {
                        chosen.push(format!("{}/{}/{}", step.name, group.name, option.name));
                    }
                }
            }
        }
        chosen
    }

    /// The default selection with the options chosen last time on again,
    /// in groups where they still are.
    #[must_use]
    pub fn selection_from(&self, chosen: &[String], present: &Present) -> Selection {
        let mut selection = self.default_selection(present);
        if chosen.is_empty() {
            return selection;
        }
        for (at, step) in self.steps.iter().enumerate() {
            for (g, group) in step.groups.iter().enumerate() {
                let names: Vec<String> = group
                    .options
                    .iter()
                    .map(|option| format!("{}/{}/{}", step.name, group.name, option.name))
                    .collect();
                if !names.iter().any(|name| chosen.contains(name)) {
                    continue;
                }
                for (o, name) in names.iter().enumerate() {
                    let on = chosen.contains(name);
                    if selection.is_on(at, g, o) != on {
                        self.toggle(&mut selection, at, g, o, on, present);
                    }
                }
            }
        }
        selection
    }
}

/// Puts a group right after a change: one at most, at least one, required
/// on, unusable off. `changed` is kept when a group needs one on.
fn settle(kind: GroupKind, picks: &mut [bool], types: &[OptionType], changed: Option<usize>) {
    for (pick, kind) in picks.iter_mut().zip(types) {
        match kind {
            OptionType::Required => *pick = true,
            OptionType::NotUsable => *pick = false,
            _ => {}
        }
    }
    if kind.single() {
        let keep = picks.iter().position(|pick| *pick);
        for (at, pick) in picks.iter_mut().enumerate() {
            *pick = Some(at) == keep;
        }
    }
    let needs_one = matches!(kind, GroupKind::ExactlyOne | GroupKind::AtLeastOne);
    if needs_one && !picks.iter().any(|pick| *pick) {
        // The one just turned off comes back on, or the first usable.
        let back = changed
            .filter(|at| types[*at] != OptionType::NotUsable)
            .or_else(|| types.iter().position(|kind| *kind != OptionType::NotUsable));
        if let Some(at) = back {
            picks[at] = true;
        }
    }
}

/// UTF-16 with a byte order mark, or UTF-8 (with or without one).
fn decode(bytes: &[u8]) -> String {
    let utf16 = |pairs: &[u8], big: bool| {
        let units: Vec<u16> = pairs
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| {
                if big {
                    u16::from_be_bytes(pair)
                } else {
                    u16::from_le_bytes(pair)
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    let text = match bytes {
        [0xFF, 0xFE, rest @ ..] => utf16(rest, false),
        [0xFE, 0xFF, rest @ ..] => utf16(rest, true),
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        // UTF-16 without a mark: every other byte of `<?xml` is zero.
        [b'<', 0, ..] => utf16(bytes, false),
        [0, b'<', ..] => utf16(bytes, true),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    };
    // The declaration may name UTF-16, which the text no longer is.
    match text.find("?>") {
        Some(end) if text.trim_start().starts_with("<?xml") => text[end + 2..].to_owned(),
        _ => text,
    }
}

fn named(node: Node, name: &str) -> bool {
    node.is_element() && node.tag_name().name().eq_ignore_ascii_case(name)
}

fn child<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    node.children().find(|child| named(*child, name))
}

fn children<'a, 'input: 'a>(
    node: Node<'a, 'input>,
    name: &'a str,
) -> impl Iterator<Item = Node<'a, 'input>> + 'a {
    node.children().filter(move |child| named(*child, name))
}

fn attribute(node: Node, name: &str) -> Option<String> {
    node.attributes()
        .find(|attribute| attribute.name().eq_ignore_ascii_case(name))
        .map(|attribute| attribute.value().to_owned())
}

fn path_attribute(node: Node, name: &str) -> Option<String> {
    attribute(node, name)
        .map(|path| normalise(&path))
        .filter(|path| !path.is_empty())
}

fn text(node: Node) -> String {
    node.descendants()
        .filter(Node::is_text)
        .filter_map(|text| text.text())
        .collect::<String>()
        .trim()
        .to_owned()
}

#[derive(Clone, Copy)]
enum Order {
    Ascending,
    Descending,
    Explicit,
}

fn order(node: Node) -> Order {
    match attribute(node, "order").as_deref() {
        Some(order) if order.eq_ignore_ascii_case("Explicit") => Order::Explicit,
        Some(order) if order.eq_ignore_ascii_case("Descending") => Order::Descending,
        _ => Order::Ascending,
    }
}

fn sort<T>(list: &mut [T], order: Order, name: impl Fn(&T) -> &str) {
    let compare = |a: &T, b: &T| name(a).to_lowercase().cmp(&name(b).to_lowercase());
    match order {
        Order::Ascending => list.sort_by(compare),
        Order::Descending => list.sort_by(|a, b| compare(b, a)),
        Order::Explicit => {}
    }
}

fn step(node: Node) -> Step {
    let groups = child(node, "optionalFileGroups")
        .map(|groups| {
            let mut list: Vec<Group> = children(groups, "group").map(group).collect();
            sort(&mut list, order(groups), |group| &group.name);
            list
        })
        .unwrap_or_default();
    Step {
        name: attribute(node, "name").unwrap_or_default(),
        visible: child(node, "visible").map(condition),
        groups,
    }
}

fn group(node: Node) -> Group {
    let kind = match attribute(node, "type")
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "selectatleastone" => GroupKind::AtLeastOne,
        "selectatmostone" => GroupKind::AtMostOne,
        "selectexactlyone" => GroupKind::ExactlyOne,
        "selectall" => GroupKind::All,
        _ => GroupKind::Any,
    };
    let options = child(node, "plugins")
        .map(|plugins| {
            let mut list: Vec<Choice> = children(plugins, "plugin").map(choice).collect();
            sort(&mut list, order(plugins), |option| &option.name);
            list
        })
        .unwrap_or_default();
    Group {
        name: attribute(node, "name").unwrap_or_default(),
        kind,
        options,
    }
}

fn choice(node: Node) -> Choice {
    let flags = child(node, "conditionFlags")
        .map(|flags| {
            children(flags, "flag")
                .filter_map(|flag| Some((attribute(flag, "name")?, text(flag))))
                .collect()
        })
        .unwrap_or_default();
    Choice {
        name: attribute(node, "name").unwrap_or_default(),
        description: child(node, "description").map(text).unwrap_or_default(),
        image: child(node, "image").and_then(|image| path_attribute(image, "path")),
        files: child(node, "files").map(installs).unwrap_or_default(),
        flags,
        kind: child(node, "typeDescriptor").map_or(
            Kind {
                default: OptionType::Optional,
                patterns: Vec::new(),
            },
            kind,
        ),
    }
}

fn option_type(node: Option<Node>) -> OptionType {
    match node
        .and_then(|node| attribute(node, "name"))
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "required" => OptionType::Required,
        "recommended" => OptionType::Recommended,
        "notusable" => OptionType::NotUsable,
        "couldbeusable" => OptionType::CouldBeUsable,
        _ => OptionType::Optional,
    }
}

fn kind(node: Node) -> Kind {
    if let Some(fixed) = child(node, "type") {
        return Kind {
            default: option_type(Some(fixed)),
            patterns: Vec::new(),
        };
    }
    let Some(depends) = child(node, "dependencyType") else {
        return Kind {
            default: OptionType::Optional,
            patterns: Vec::new(),
        };
    };
    Kind {
        default: option_type(child(depends, "defaultType")),
        patterns: child(depends, "patterns")
            .map(|patterns| {
                children(patterns, "pattern")
                    .map(|pattern| {
                        (
                            child(pattern, "dependencies").map_or(Condition::Holds, condition),
                            option_type(child(pattern, "type")),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// A composite condition: its `operator` over the conditions inside it.
fn condition(node: Node) -> Condition {
    let parts: Vec<Condition> = node
        .children()
        .filter(Node::is_element)
        .filter_map(|part| {
            let name = part.tag_name().name().to_lowercase();
            match name.as_str() {
                "dependencies" => Some(condition(part)),
                "flagdependency" => Some(Condition::Flag {
                    name: attribute(part, "flag").unwrap_or_default(),
                    value: attribute(part, "value").unwrap_or_default(),
                }),
                "filedependency" => Some(Condition::File {
                    file: attribute(part, "file").unwrap_or_default(),
                    state: match attribute(part, "state")
                        .unwrap_or_default()
                        .to_lowercase()
                        .as_str()
                    {
                        "missing" => FileState::Missing,
                        "inactive" => FileState::Inactive,
                        _ => FileState::Active,
                    },
                }),
                _ if name.ends_with("dependency") => Some(Condition::Holds),
                _ => None,
            }
        })
        .collect();
    let any =
        attribute(node, "operator").is_some_and(|operator| operator.eq_ignore_ascii_case("Or"));
    if any {
        Condition::Any(parts)
    } else {
        Condition::All(parts)
    }
}

fn installs(node: Node) -> Vec<Install> {
    node.children()
        .filter(|item| named(*item, "file") || named(*item, "folder"))
        .map(|item| Install {
            source: attribute(item, "source")
                .map(|source| normalise(&source))
                .unwrap_or_default(),
            destination: attribute(item, "destination").map(|path| normalise(&path)),
            into_folder: attribute(item, "destination")
                .is_some_and(|path| path.trim().is_empty() || path.ends_with(['/', '\\'])),
            folder: named(item, "folder"),
            priority: attribute(item, "priority")
                .and_then(|priority| priority.trim().parse().ok())
                .unwrap_or(0),
        })
        .collect()
}

/// A path as the file writes it (`Data\Textures\`, `.\x`) with `/`
/// between parts and none at either end.
#[must_use]
pub fn normalise(path: &str) -> String {
    path.replace('\\', "/")
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>()
        .join("/")
}

impl Install {
    /// Where its file goes, or the folder's contents, from the mod's
    /// folder; `None` when the path climbs out (`..`).
    #[must_use]
    pub fn target(&self) -> Option<String> {
        let target = match &self.destination {
            None => self.source.clone(),
            Some(folder) if !self.folder && self.into_folder => {
                let name = self.source.rsplit('/').next().unwrap_or_default();
                normalise(&format!("{folder}/{name}"))
            }
            Some(destination) => destination.clone(),
        };
        safe(&target).then_some(target)
    }
}

/// A relative path that stays inside (no `..`, not absolute, no drive).
#[must_use]
pub fn safe(path: &str) -> bool {
    !path.contains(':') && path.split('/').all(|part| part != "..")
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = r#"<?xml version="1.0" encoding="UTF-16"?>
<config xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <moduleName> Cool Mod </moduleName>
  <moduleImage path="fomod\images\cover.png"/>
  <moduleDependencies operator="And">
    <fileDependency file="Skyrim.esm" state="Active"/>
  </moduleDependencies>
  <requiredInstallFiles>
    <folder source="Core" destination=""/>
  </requiredInstallFiles>
  <installSteps order="Explicit">
    <installStep name="Body">
      <optionalFileGroups>
        <group name="Choose a body" type="SelectExactlyOne">
          <plugins order="Explicit">
            <plugin name="Slim">
              <description>A slim one.</description>
              <image path="fomod/slim.png"/>
              <files><folder source="Bodies\Slim" destination="meshes\body"/></files>
              <conditionFlags><flag name="body">slim</flag></conditionFlags>
              <typeDescriptor><type name="Optional"/></typeDescriptor>
            </plugin>
            <plugin name="Strong">
              <description>A strong one.</description>
              <files><folder source="Bodies\Strong" destination="meshes\body"/></files>
              <conditionFlags><flag name="body">strong</flag></conditionFlags>
              <typeDescriptor><type name="Recommended"/></typeDescriptor>
            </plugin>
          </plugins>
        </group>
        <group name="Extras" type="SelectAny">
          <plugins>
            <plugin name="Zz last by name">
              <files><file source="extras\z.txt" destination="docs\"/></files>
              <typeDescriptor><type name="Optional"/></typeDescriptor>
            </plugin>
            <plugin name="Aa first by name">
              <files><file source="extras\a.txt" priority="5"/></files>
              <typeDescriptor><type name="NotUsable"/></typeDescriptor>
            </plugin>
          </plugins>
        </group>
      </optionalFileGroups>
    </installStep>
    <installStep name="Strong only">
      <visible><flagDependency flag="body" value="strong"/></visible>
      <optionalFileGroups>
        <group name="Muscles" type="SelectAtLeastOne">
          <plugins>
            <plugin name="Big">
              <files><file source="extras\big.txt" destination="big.txt" priority="2"/></files>
              <typeDescriptor>
                <dependencyType>
                  <defaultType name="Optional"/>
                  <patterns>
                    <pattern>
                      <dependencies operator="Or">
                        <fileDependency file="Data\Strong.esp" state="Active"/>
                        <flagDependency flag="body" value="strong"/>
                      </dependencies>
                      <type name="Required"/>
                    </pattern>
                  </patterns>
                </dependencyType>
              </typeDescriptor>
            </plugin>
          </plugins>
        </group>
      </optionalFileGroups>
    </installStep>
  </installSteps>
  <conditionalFileInstalls>
    <patterns>
      <pattern>
        <dependencies operator="And">
          <flagDependency flag="body" value="slim"/>
          <fileDependency file="Missing.esp" state="Missing"/>
        </dependencies>
        <files><file source="patches\slim.esp" destination="Slim Patch.esp"/></files>
      </pattern>
    </patterns>
  </conditionalFileInstalls>
</config>"#;

    fn utf16(text: &str) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xFE];
        for unit in text.encode_utf16() {
            bytes.extend(unit.to_le_bytes());
        }
        bytes
    }

    fn targets(files: &[Install]) -> Vec<String> {
        files.iter().filter_map(Install::target).collect()
    }

    #[test]
    fn reads_an_installer_in_utf16_in_its_order() {
        let installer = Installer::parse(&utf16(CONFIG)).unwrap();
        assert_eq!(installer, Installer::parse(CONFIG.as_bytes()).unwrap());
        assert_eq!(installer.name, "Cool Mod");
        assert_eq!(installer.image.as_deref(), Some("fomod/images/cover.png"));
        let names: Vec<&str> = installer
            .steps
            .iter()
            .map(|step| step.name.as_str())
            .collect();
        assert_eq!(
            names,
            ["Body", "Strong only"],
            "Explicit keeps the file's order"
        );
        let groups = &installer.steps[0].groups;
        // Groups default to Ascending: "Choose a body" before "Extras".
        assert_eq!(groups[0].name, "Choose a body");
        assert_eq!(groups[0].options[0].name, "Slim", "Explicit");
        assert_eq!(groups[1].options[0].name, "Aa first by name", "Ascending");
        assert_eq!(groups[0].options[0].description, "A slim one.");
        assert_eq!(
            groups[0].options[0].image.as_deref(),
            Some("fomod/slim.png")
        );
        assert!(Installer::parse(b"<notfomod/>").is_err());
        assert!(Installer::parse(b"not xml").is_err());
    }

    #[test]
    fn follows_choices_through_flags_steps_and_conditions() {
        let installer = Installer::parse(CONFIG.as_bytes()).unwrap();
        let present = Present::default();
        let mut selection = installer.default_selection(&present);
        // Strong is recommended, so on; the unusable extra stays off.
        assert_eq!(selection.group(0, 0), [false, true]);
        assert_eq!(selection.group(0, 1), [false, false]);
        assert_eq!(installer.visible_steps(&selection, &present), [0, 1]);
        // Required in the second step, because body is strong.
        assert_eq!(
            installer.types(&selection, 1, &present)[0],
            [OptionType::Required]
        );
        assert_eq!(selection.group(1, 0), [true]);
        assert_eq!(
            targets(&installer.files(&selection, &present)),
            ["", "meshes/body", "big.txt"],
            "lowest priority first"
        );

        // Exactly one: choosing Slim turns Strong off and hides step two.
        installer.toggle(&mut selection, 0, 0, 0, true, &present);
        assert_eq!(selection.group(0, 0), [true, false]);
        installer.toggle(&mut selection, 0, 0, 0, false, &present);
        assert_eq!(selection.group(0, 0), [true, false], "one must stay on");
        assert_eq!(installer.visible_steps(&selection, &present), [0]);
        // Not usable never turns on; any other extra does.
        installer.toggle(&mut selection, 0, 1, 0, true, &present);
        installer.toggle(&mut selection, 0, 1, 1, true, &present);
        assert_eq!(selection.group(0, 1), [false, true]);
        let files = installer.files(&selection, &present);
        assert_eq!(
            targets(&files),
            ["", "meshes/body", "docs/z.txt", "Slim Patch.esp"],
            "the hidden step's files are left out; the slim patch comes in"
        );

        let chosen = installer.chosen(&selection, &present);
        assert_eq!(
            chosen,
            ["Body/Choose a body/Slim", "Body/Extras/Zz last by name"]
        );
        let again = installer.selection_from(&chosen, &present);
        assert_eq!(again.step(0), selection.step(0));

        // The patch needs Missing.esp to be missing.
        let present = Present {
            files: ["missing.esp".to_owned()].into(),
            ..Present::default()
        };
        let files = installer.files(&selection, &present);
        assert!(!targets(&files).contains(&"Slim Patch.esp".to_owned()));
        assert!(
            installer
                .needs
                .as_ref()
                .is_some_and(|needs| !needs.holds(&HashMap::new(), &present))
        );
    }

    #[test]
    fn a_selection_ignores_places_its_installer_lacks() {
        let installer = Installer::parse(
            br#"<config><installSteps><installStep name="One"><optionalFileGroups>
                <group name="Pick" type="SelectAny"><plugins>
                  <plugin name="A"><typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
                </plugins></group></optionalFileGroups></installStep></installSteps></config>"#,
        )
        .unwrap();
        let present = Present::default();
        let mut selection = installer.default_selection(&present);
        let before = selection.clone();
        installer.toggle(&mut selection, 0, 0, 5, true, &present);
        installer.toggle(&mut selection, 0, 3, 0, true, &present);
        installer.toggle(&mut selection, 9, 0, 0, true, &present);
        assert_eq!(selection, before);
        assert_eq!(
            installer.types(&selection, 9, &present),
            Vec::<Vec<OptionType>>::new()
        );
        assert!(!selection.is_on(4, 4, 4));
        assert_eq!(selection.group(0, 7), [] as [bool; 0]);
        installer.toggle(&mut selection, 0, 0, 0, true, &present);
        assert!(selection.is_on(0, 0, 0));
    }

    #[test]
    fn a_plugin_that_is_off_is_inactive_not_active() {
        let file = |file: &str, state| Condition::File {
            file: file.into(),
            state,
        };
        let present = Present {
            files: ["on.esp", "off.esp", "textures/a.dds"]
                .map(str::to_owned)
                .into(),
            inactive: ["off.esp".to_owned()].into(),
        };
        let flags = HashMap::new();
        let holds = |condition: Condition| condition.holds(&flags, &present);
        assert!(holds(file("On.esp", FileState::Active)));
        assert!(!holds(file("On.esp", FileState::Inactive)));
        assert!(holds(file("Off.esp", FileState::Inactive)));
        assert!(!holds(file("Off.esp", FileState::Active)));
        assert!(!holds(file("Off.esp", FileState::Missing)));
        assert!(holds(file("textures\\a.dds", FileState::Active)));
        assert!(holds(file("Gone.esp", FileState::Missing)));
        assert!(!holds(file("Gone.esp", FileState::Inactive)));
    }

    #[test]
    fn a_choice_can_make_an_option_in_the_same_step_required() {
        let installer = Installer::parse(
            br#"<config><installSteps><installStep name="One"><optionalFileGroups order="Explicit">
                <group name="Pick" type="SelectExactlyOne"><plugins order="Explicit">
                  <plugin name="A"><conditionFlags><flag name="a">On</flag></conditionFlags>
                    <typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
                  <plugin name="B"><conditionFlags><flag name="b">On</flag></conditionFlags>
                    <typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
                </plugins></group>
                <group name="Results" type="SelectAny"><plugins order="Explicit">
                  <plugin name="For A"><files><file source="a.txt"/></files><typeDescriptor><dependencyType>
                    <defaultType name="NotUsable"/><patterns><pattern><dependencies>
                    <flagDependency flag="a" value="On"/></dependencies><type name="Required"/></pattern></patterns>
                  </dependencyType></typeDescriptor></plugin>
                  <plugin name="For B"><files><file source="b.txt"/></files><typeDescriptor><dependencyType>
                    <defaultType name="NotUsable"/><patterns><pattern><dependencies>
                    <flagDependency flag="b" value="On"/></dependencies><type name="Required"/></pattern></patterns>
                  </dependencyType></typeDescriptor></plugin>
                </plugins></group>
            </optionalFileGroups></installStep></installSteps></config>"#,
        )
        .unwrap();
        let present = Present::default();
        let mut selection = installer.default_selection(&present);
        assert_eq!(selection.step(0), [vec![true, false], vec![true, false]]);
        assert_eq!(targets(&installer.files(&selection, &present)), ["a.txt"]);
        installer.toggle(&mut selection, 0, 0, 1, true, &present);
        assert_eq!(selection.step(0), [vec![false, true], vec![false, true]]);
        assert_eq!(targets(&installer.files(&selection, &present)), ["b.txt"]);
    }

    #[test]
    fn keeps_paths_inside_the_mod() {
        let install = |source: &str, destination: Option<&str>, folder: bool| Install {
            source: normalise(source),
            destination: destination.map(normalise),
            into_folder: destination.is_some_and(|d| d.is_empty() || d.ends_with(['/', '\\'])),
            folder,
            priority: 0,
        };
        assert_eq!(
            install("a\\b.esp", None, false).target().as_deref(),
            Some("a/b.esp")
        );
        assert_eq!(
            install("a\\b.esp", Some(""), false).target().as_deref(),
            Some("b.esp")
        );
        assert_eq!(
            install("a\\b.esp", Some("x\\"), false).target().as_deref(),
            Some("x/b.esp")
        );
        assert_eq!(
            install("Core", Some(""), true).target().as_deref(),
            Some("")
        );
        assert_eq!(install("a.esp", Some("..\\..\\evil"), false).target(), None);
        assert_eq!(
            install("a.esp", Some("C:\\Windows\\x"), false).target(),
            None
        );
        assert_eq!(normalise(".\\Data\\\\Textures\\"), "Data/Textures");
    }

    /// Reads every `*.xml` in a folder, as in `ZENDO_TEST_FOMODS=<folder>
    /// cargo test -p zendo-core fomod -- --ignored --nocapture`.
    #[test]
    #[ignore = "reads real installers from the folder in ZENDO_TEST_FOMODS"]
    fn reads_real_installers() {
        let folder = std::env::var("ZENDO_TEST_FOMODS").expect("ZENDO_TEST_FOMODS");
        let mut read = 0;
        for entry in std::fs::read_dir(folder).unwrap().flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|extension| extension != "xml") {
                continue;
            }
            let installer = Installer::parse(&std::fs::read(&path).unwrap())
                .unwrap_or_else(|err| panic!("{}: {err:#}", path.display()));
            let present = Present::default();
            let selection = installer.default_selection(&present);
            let files = installer.files(&selection, &present);
            println!(
                "{}: {:?}, {} steps, {} shown, {} files by default, {} options",
                path.display(),
                installer.name,
                installer.steps.len(),
                installer.visible_steps(&selection, &present).len(),
                files.len(),
                installer
                    .steps
                    .iter()
                    .flat_map(|step| &step.groups)
                    .map(|group| group.options.len())
                    .sum::<usize>(),
            );
            read += 1;
        }
        assert_ne!(read, 0);
    }
}
