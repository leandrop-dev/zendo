// The typed boundary to Rust: each function wraps one command, and the
// types match the structs the commands return.

import { invoke } from '@tauri-apps/api/core';

// A language the window speaks.
export type Language = { tag: string; name: string };

// The window's colours, as CSS colours (the Rust side sends hex).
export type Palette = {
	background: string;
	sidebar: string;
	surface: string;
	raised: string;
	line: string;
	foreground: string;
	dim: string;
	accent: string;
	good: string;
	bad: string;
	/// Text drawn on the accent colour.
	on_accent: string;
	light: boolean;
};

// What the shell starts with.
export type Bootstrap = {
	version: string;
	// "Version 1.0.1", in the window's words.
	version_line: string;
	// Where the source is, for About's buttons.
	repository: string;
	language: string;
	languages: Language[];
	/// The current language's messages that take no values, by id.
	messages: Record<string, string>;
	palette: Palette;
	fullscreen: boolean;
	/// Whether closing the window hides it to the tray.
	tray: boolean;
	// The FPS harness is on (ZENDO_FPS in the environment).
	fps: boolean;
};

// True inside the Tauri window. In a plain browser (`vite dev` on its own)
// the IPC is not there, so the pages say so and use Zendo's own colours.
export function inTauri(): boolean {
	return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

// The words, the colours and the window's state, once at start.
export async function bootstrap(): Promise<Bootstrap> {
	return invoke<Bootstrap>('bootstrap');
}

// A message with values: `tr('count-games', { count: 3 })`.
export async function tr(id: string, args: Record<string, string | number> = {}): Promise<string> {
	return invoke<string>('tr', { id, args });
}

// Full screen on or off, and which it is now.
export async function toggleFullscreen(): Promise<boolean> {
	return invoke<boolean>('window_fullscreen_toggle');
}

// A game's card, as the grid draws it.
export type Card = {
	id: string;
	title: string;
	caption: string;
	caption_accent: boolean;
	faded: boolean;
	badge: string | null;
	badge_running: boolean;
	// The share of achievements, 0 to 1; null when the game has none.
	progress: number | null;
	marked: boolean;
	// The cover's path in the cache.
	cover: string | null;
};

// A library chip: the view, a source, an order, or an organize action.
export type Chip = {
	key: string;
	label: string;
	count: number | null;
	chosen: boolean;
};

export type Chips = {
	views: Chip[];
	sources: Chip[];
	sorts: Chip[];
	organize: Chip[];
	// The bulk chips, while games are marked.
	bulk: Chip[];
	// "3 marked", while any are.
	marked: string | null;
};

// What the page says in place of the grid.
export type Empty = { title: string; about: string | null };

// The Library: every game's card once, and the ones the filter shows.
export type LibraryList = {
	ready: boolean;
	cards: Card[];
	shown: string[];
	chips: Chips;
	cursor: number;
	empty: Empty | null;
	status: string | null;
	search: string;
	marked: string[];
};

// The Library after a filter, a sort, a search or a cursor move.
export type LibraryFiltered = Omit<LibraryList, 'cards'>;

// The Library.

export async function libraryList(): Promise<LibraryList> {
	return invoke<LibraryList>('library_list');
}

export async function libraryPick(key: string): Promise<LibraryFiltered> {
	return invoke<LibraryFiltered>('library_pick', { key });
}

export async function librarySearch(text: string): Promise<LibraryFiltered> {
	return invoke<LibraryFiltered>('library_search', { text });
}

export async function libraryCycleSort(): Promise<LibraryFiltered> {
	return invoke<LibraryFiltered>('library_cycle_sort');
}

export async function libraryClick(id: string): Promise<LibraryFiltered> {
	return invoke<LibraryFiltered>('library_click', { id });
}

// left, right, up, down, page-up, page-down, first or last.
export async function libraryStep(step: string): Promise<LibraryFiltered> {
	return invoke<LibraryFiltered>('library_step', { step });
}

// The grid's columns and height, so paging and the cursor know the grid.
export async function libraryFit(columns: number, height: number): Promise<LibraryFiltered> {
	return invoke<LibraryFiltered>('library_fit', { columns, height });
}

export async function libraryBack(): Promise<LibraryFiltered> {
	return invoke<LibraryFiltered>('library_back');
}

export async function libraryShowHidden(): Promise<LibraryFiltered> {
	return invoke<LibraryFiltered>('library_show_hidden');
}

// Marks a game, or leaves it; nothing is the card under the cursor.
export async function libraryToggleMark(id?: string): Promise<LibraryFiltered> {
	return invoke<LibraryFiltered>('library_toggle_mark', { id: id ?? null });
}

// A game's page: the header, the tiles, the statuses and how it is running.
export type GamePage = {
	id: string;
	title: string;
	source_line: string;
	cover: string | null;
	hero: string | null;
	pinned: boolean;
	hidden: boolean;
	status: string | null;
	// 0 idle (Play), 1 starting, 2 running (Stop), 3 stopping.
	run_state: number;
	line: string;
	tiles: { label: string; value: string }[];
	statuses: { key: string; label: string; chosen: boolean }[];
	// A game added by hand: its page edits and removes it.
	local: boolean;
	details: (Fact & { tone: number })[];
	// The session counts and lengths, empty until the game has one.
	sessions: (Fact & { tone: number })[];
	// The latest sessions, by date.
	recent: (Fact & { tone: number })[];
	// The DLC installed with the game, when its engine records any.
	dlc: { title: string }[];
	// The Wine prefix card, for a Windows game Zendo starts.
	prefix: {
		folder: string | null;
		fixes: string[];
		gog: boolean;
		// No tool runs in the prefix and the game does not.
		free: boolean;
		// The components typed last.
		components: string;
		// What runs, or how the last tool went.
		status: { text: string; tone: number } | null;
	} | null;
};

// A line of a card: a label and its value.
export type Fact = { label: string; value: string; path: boolean };

// A badge on the header: ProtonDB's rating.
export type Pill = {
	label: string;
	// 0 takes `tint`; 1, 2 and 3 take the theme's good, bad and dim.
	tone: number;
	tint: string | null;
	tip: string;
	url: string;
};

// A line of a card, and the link its value opens.
export type Row = { label: string; value: string; url: string | null };

// What the stores and the sites say about a game, as its page's cards.
export type GameCards = {
	// While ProtonDB is asked, and once its rating is known.
	pill: Pill | null;
	// Steam's about line, empty when its store has no page.
	about: string;
	// No Steam game was found for this one.
	no_match: string | null;
	shots: { thumb: string; full: string }[];
	// What only Steam says, for the details card.
	steam: Row[];
	requirements: {
		source: string;
		both: boolean;
		rows: { label: string; minimum: string; recommended: string }[];
	};
	achievements: {
		summary: string;
		// The share of them unlocked, 0 to 1; null when it cannot be said.
		share: number | null;
		rows: {
			icon: string | null;
			name: string;
			description: string;
			side: string;
			unlocked: boolean;
		}[];
		more: string | null;
	};
	// GOG's notes for each update: 0 a title, 1 a heading, 2 a paragraph,
	// 3 a list item, 4 a gap.
	changelog: { lines: { kind: number; text: string }[]; more: string | null };
	// The cards that could not be asked, with why.
	failures: Row[];
	// The Technical card's Steam app id row.
	steam_app: { line: string; input: string; clearable: boolean };
};

export async function cardsGet(id: string): Promise<GameCards> {
	return invoke<GameCards>('cards_get', { id });
}

export async function cardsRetry(id: string): Promise<GameCards> {
	return invoke<GameCards>('cards_retry', { id });
}

export async function cardsMoreAchievements(id: string): Promise<GameCards> {
	return invoke<GameCards>('cards_more_achievements', { id });
}

export async function cardsMoreChangelog(id: string): Promise<GameCards> {
	return invoke<GameCards>('cards_more_changelog', { id });
}

// Gives the game a Steam app id, or takes it back with null and answers the
// line the page shows.
export async function steamAppSet(id: string, app: string | null): Promise<string> {
	return invoke<string>('steam_app_set', { id, app });
}

// Opens a link with the desktop's browser.
export async function openLink(url: string): Promise<void> {
	return invoke<void>('open_link', { url });
}

export async function gameGet(id: string): Promise<GamePage> {
	return invoke<GamePage>('game_get', { id });
}

export async function gameLaunch(id: string): Promise<GamePage> {
	return invoke<GamePage>('game_launch', { id });
}

export async function gameStop(id: string): Promise<GamePage> {
	return invoke<GamePage>('game_stop', { id });
}

export async function gameSetPinned(id: string, pinned: boolean): Promise<GamePage> {
	return invoke<GamePage>('game_set_pinned', { id, pinned });
}

export async function gameSetHidden(id: string, hidden: boolean): Promise<GamePage> {
	return invoke<GamePage>('game_set_hidden', { id, hidden });
}

export async function gameSetStatus(id: string, status: string | null): Promise<GamePage> {
	return invoke<GamePage>('game_set_status', { id, status });
}

// A game added by hand: what the form holds.
export type LocalDraft = {
	id: string | null;
	title: string;
	program: string;
	arguments: string;
};

export async function localGet(id: string): Promise<LocalDraft> {
	return invoke<LocalDraft>('local_get', { id });
}

// What the form says under the program field: a Linux program or a Windows
// one, which runs through Proton.
export async function localAbout(program: string): Promise<string> {
	return invoke<string>('local_about', { program });
}

// The Choose button; null when the dialog was closed.
export async function localPickProgram(): Promise<string | null> {
	return invoke<string | null>('local_pick_program');
}

// Adds the game, or saves it, and answers its id.
export async function localSave(draft: LocalDraft): Promise<string> {
	return invoke<string>('local_save', { draft });
}

export async function localRemove(id: string): Promise<void> {
	return invoke<void>('local_remove', { id });
}

// Opens the Wine prefix's folder with the desktop's file manager.
export async function prefixOpen(path: string): Promise<void> {
	return invoke<void>('prefix_open', { path });
}

// Runs a tool in the game's prefix: wine-settings, winetricks, gog-setup or
// known-fixes.
export async function prefixRun(id: string, tool: string): Promise<void> {
	return invoke<void>('prefix_run', { id, tool });
}

// Installs the winetricks components typed.
export async function prefixInstall(id: string, typed: string): Promise<void> {
	return invoke<void>('prefix_install', { id, typed });
}

// Run a program…: the file dialog, then the program in the prefix.
export async function prefixPickProgram(id: string): Promise<void> {
	return invoke<void>('prefix_pick_program', { id });
}

// Asks for the known fixes' list when it is a day old, then says the card
// may be read again.
export async function prefixWant(id: string): Promise<void> {
	return invoke<void>('prefix_want', { id });
}

// The Mods card: the mods Zendo keeps for a game, the profile in use, a
// Bethesda game's plugins, the folder they go in, and Apply.
export type ModsCard = {
	// The card is drawn: the mods are here, or the read failed with why.
	show: boolean;
	// 1 the read failed, 2 the mods are here.
	body_kind: number;
	message: string;
	// What is happening to the game's mods now; empty when nothing is.
	busy: string;
	// No change is in flight, so the controls take presses.
	free: boolean;
	// The game is on Nexus Mods.
	nexus: boolean;
	// The line the last change left.
	note: string | null;
	profiles: { label: string; chosen: boolean }[];
	delete_profile_label: string;
	delete_profile_asking: boolean;
	delete_profile_on: boolean;
	naming: { placeholder: string; name: string; ok: boolean } | null;
	place: string;
	target: string;
	mods: {
		id: string;
		name: string;
		facts: string;
		enabled: boolean;
		up: boolean;
		down: boolean;
		delete_label: string;
		asking: boolean;
	}[];
	plugins: {
		note: string;
		note_bad: boolean;
		file: string | null;
		rows: {
			name: string;
			facts: string;
			problems: string;
			enabled: boolean;
			locked: boolean;
			up: boolean;
			down: boolean;
		}[];
	} | null;
	overwrite: string | null;
	overwrite_label: string;
	overwrite_asking: boolean;
	folder: string;
	folder_editable: boolean;
	folder_save: boolean;
	folder_known: boolean;
	state: string;
	state_tone: number;
	can_apply: boolean;
	can_restore: boolean;
};

export async function modsGet(id: string): Promise<ModsCard> {
	return invoke<ModsCard>('mods_get', { id });
}

// The page opened: reads the game's mods again.
export async function modsBegin(id: string): Promise<ModsCard> {
	return invoke<ModsCard>('mods_begin', { id });
}

export async function modsToggleMod(id: string, modId: string, on: boolean): Promise<ModsCard> {
	return invoke<ModsCard>('mods_toggle_mod', { id, modId, on });
}

export async function modsMoveMod(id: string, modId: string, up: boolean): Promise<ModsCard> {
	return invoke<ModsCard>('mods_move_mod', { id, modId, up });
}

export async function modsTogglePlugin(id: string, name: string, on: boolean): Promise<ModsCard> {
	return invoke<ModsCard>('mods_toggle_plugin', { id, name, on });
}

export async function modsMovePlugin(id: string, name: string, up: boolean): Promise<ModsCard> {
	return invoke<ModsCard>('mods_move_plugin', { id, name, up });
}

export async function modsRemove(id: string, modId: string): Promise<ModsCard> {
	return invoke<ModsCard>('mods_remove', { id, modId });
}

export async function modsClearOverwrite(id: string): Promise<ModsCard> {
	return invoke<ModsCard>('mods_clear_overwrite', { id });
}

export async function modsFolderSave(id: string, text: string): Promise<ModsCard> {
	return invoke<ModsCard>('mods_folder_save', { id, text });
}

// Back to where the game is known to take its mods.
export async function modsFolderKnown(id: string): Promise<ModsCard> {
	return invoke<ModsCard>('mods_folder_known', { id });
}

export async function modsProfileSwitch(id: string, name: string): Promise<ModsCard> {
	return invoke<ModsCard>('mods_profile_switch', { id, name });
}

// New… (renaming false) or Rename… (true): the name field opens.
export async function modsProfileNaming(id: string, renaming: boolean): Promise<ModsCard> {
	return invoke<ModsCard>('mods_profile_naming', { id, renaming });
}

export async function modsProfileSave(id: string, name: string): Promise<ModsCard> {
	return invoke<ModsCard>('mods_profile_save', { id, name });
}

export async function modsProfileCancel(id: string): Promise<ModsCard> {
	return invoke<ModsCard>('mods_profile_cancel', { id });
}

export async function modsProfileDelete(id: string): Promise<ModsCard> {
	return invoke<ModsCard>('mods_profile_delete', { id });
}

// Apply, or back to vanilla with false; never while the game runs.
export async function modsApply(id: string, apply: boolean): Promise<ModsCard> {
	return invoke<ModsCard>('mods_apply', { id, apply });
}

// Add mod…: the file dialog, then the archive's unpacking on a thread.
export async function modsAdd(id: string): Promise<void> {
	return invoke<void>('mods_add', { id });
}

// Opens the game's page on Nexus Mods in the browser.
export async function modsFindNexus(id: string): Promise<void> {
	return invoke<void>('mods_find_nexus', { id });
}

// A line of a Settings list: a source, or the report's answer.
export type SettingsRow = { label: string; value: string; tone: number };

// An API key's field: the value, the buttons and what it last said.
export type SettingsKey = {
	value: string;
	shown: boolean;
	checking: boolean;
	// The field holds another key than the one saved.
	changed: boolean;
	note: SettingsRow | null;
	kept: string;
	kept_bad: boolean;
};

// The Nexus key's field, with whose key it is and what opens links.
export type SettingsNexusKey = SettingsKey & {
	handler_line: string;
	handler_good: boolean;
	handler_can: boolean;
};

// A thing Settings lists with buttons: a launcher, an engine, a Proton.
export type SettingsSetRow = {
	title: string;
	sub: string;
	path: string;
	note: string;
	note_tone: number;
	action: string;
	action_on: boolean;
	remove: string;
	remove_on: boolean;
	remove_danger: boolean;
	status: string;
	status_tone: number;
};

// A download in progress: its name, the bytes so far and how far it is.
export type SettingsBar = {
	active: boolean;
	name: string;
	size: string;
	percent: number;
	stopping: boolean;
};

// The EOS overlay's card: its state and its Install/Remove buttons.
export type SettingsOverlay = {
	status: string;
	install: string;
	install_on: boolean;
	remove: string;
	remove_on: boolean;
	note: SettingsRow | null;
};

// The Proton group: the default, the ones found, and the releases on offer.
export type SettingsProtons = {
	defaults: { name: string | null; label: string; chosen: boolean }[];
	installed: SettingsSetRow[];
	installed_none: string;
	builds: { label: string; chosen: boolean }[];
	releases_title: string;
	releases: SettingsSetRow[];
	refresh_on: boolean;
	line: SettingsRow | null;
	older: boolean;
	bar: SettingsBar;
	note: SettingsRow | null;
};

// One store's card on the Accounts page.
export type AccountCard = {
	badge: string;
	title: string;
	status: string;
	status_tone: number;
	engine: string;
	engine_ok: boolean;
	action: string;
	// 0 Sign in, 1 Sign out.
	action_kind: number;
	action_on: boolean;
	gog: boolean;
	// 0 none, 1 in Zendo's window, 2 in the browser.
	flow: number;
	line: string;
	note: string;
	retry: boolean;
	browser_on: boolean;
	step_one: string;
	step_two: string;
	step_three: string;
	open_label: string;
	open_on: boolean;
	finish_label: string;
	finish_on: boolean;
};

export type SteamCard = {
	// 0 signed out, 1 signing in, 2 signed in.
	state: number;
	status: string;
	about: string;
	family: string;
	note: string;
	note_good: boolean;
	family_note: string;
	kept: string;
	kept_bad: boolean;
};

export type LauncherCard = {
	badge: string;
	title: string;
	about: string;
	status: string;
	status_tone: number;
	path: string;
	line: string;
	action: string;
	action_on: boolean;
	note: string;
	note_tone: number;
};

// The Accounts page: Steam's card, the stores, and Nexus Launcher's.
export type AccountsPage = {
	steam: SteamCard;
	cards: AccountCard[];
	xbox: LauncherCard;
	gog_playtime: boolean;
	gog_galaxy: boolean;
	code: string;
};

// One collection in the sheet: hand-picked or a saved view.
export type CollRow = {
	name: string;
	sub: string;
	count: string;
	view: boolean;
	checked: boolean;
	checkable: boolean;
	renaming: boolean;
	confirm: boolean;
};

// The Collections sheet.
export type CollectionsPage = {
	open: boolean;
	count: string;
	empty: string;
	now: string;
	draft: string;
	rename_draft: string;
	note: SettingsRow | null;
	rows: CollRow[];
};

// The Settings page: General, Library and Diagnostics.
export type SettingsPage = {
	languages: { tag: string | null; label: string; chosen: boolean }[];
	themes: { tag: string; label: string; chosen: boolean }[];
	theme_about: string;
	theme_note: SettingsRow | null;
	theme_problems: string[];
	tray: boolean;
	discord: boolean;
	steamgriddb: SettingsKey;
	nexus: SettingsNexusKey;
	covers: boolean;
	// Downloads can start now: the setting is on and nothing runs.
	covers_now: boolean;
	auto_update: boolean;
	workers: { count: number | null; label: string; chosen: boolean }[];
	sources: SettingsRow[];
	launchers: SettingsSetRow[];
	launchers_bar: SettingsBar;
	path_first: boolean;
	engines: SettingsSetRow[];
	engines_bar: SettingsBar;
	engines_note: SettingsRow | null;
	runtimes: SettingsSetRow[];
	runtimes_note: SettingsRow | null;
	overlay: SettingsOverlay;
	proton: SettingsProtons;
	files: { label: string; value: string; open: string | null }[];
	report_busy: boolean;
	report_note: SettingsRow | null;
	logs: string | null;
};

export async function settingsGet(): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_get');
}

// null follows the desktop's language.
export async function settingsSetLanguage(tag: string | null): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_set_language', { language: tag });
}

// A theme file's id, or "system" for the KDE scheme.
export async function settingsSetTheme(tag: string | null): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_set_theme', { theme: tag });
}

export async function settingsSetTray(on: boolean): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_set_tray', { on });
}

export async function settingsSetDiscord(on: boolean): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_set_discord', { on });
}

export async function settingsSetCovers(on: boolean): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_set_covers', { on });
}

export async function settingsSetAutoUpdate(on: boolean): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_set_auto_update', { on });
}

// null is the engine's own choice.
export async function settingsSetWorkers(count: number | null): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_set_workers', { workers: count });
}

// Gathers the report and copies it, on a thread.
// Fetches the covers the library is missing.
export async function settingsDownloadCovers(): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_download_covers');
}

export async function settingsCopyReport(): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_copy_report');
}

export async function settingsOpen(path: string): Promise<void> {
	return invoke<void>('settings_open', { path });
}

// The SteamGridDB key's field.
export async function settingsKeyEdited(text: string): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_key_edited', { text });
}

export async function settingsKeyToggleShown(): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_key_toggle_shown');
}

export async function settingsKeyCheck(): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_key_check');
}

export async function settingsKeySave(): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_key_save');
}

export async function settingsKeyGet(): Promise<void> {
	return invoke<void>('settings_key_get');
}

// The Nexus key's field, and the handler that opens its links.
export async function settingsNexusKeyEdited(text: string): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_nexus_key_edited', { text });
}

export async function settingsNexusKeyToggleShown(): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_nexus_key_toggle_shown');
}

export async function settingsNexusKeyCheck(): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_nexus_key_check');
}

export async function settingsNexusKeySave(): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_nexus_key_save');
}

export async function settingsNexusKeyGet(): Promise<void> {
	return invoke<void>('settings_nexus_key_get');
}

export async function settingsNexusBecomeHandler(): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_nexus_become_handler');
}

// The Launchers group: a launcher's one button, and Stop on the download.
export async function settingsLauncherAct(index: number): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_launcher_act', { index });
}

export async function settingsLaunchersStop(): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_launchers_stop');
}

// The Engines group: the engines, the runtimes and the EOS overlay.
export async function settingsSetPathFirst(on: boolean): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_set_path_first', { on });
}

export async function settingsEnginesCheck(): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_engines_check');
}

export async function settingsEngineAct(index: number): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_engine_act', { index });
}

export async function settingsEngineRemove(index: number): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_engine_remove', { index });
}

export async function settingsEnginesStop(): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_engines_stop');
}

export async function settingsRuntimeAct(index: number): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_runtime_act', { index });
}

export async function settingsRuntimeRemove(index: number): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_runtime_remove', { index });
}

export async function settingsOverlayDoInstall(): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_overlay_do_install');
}

export async function settingsOverlayDoRemove(): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_overlay_do_remove');
}

// The Proton group.
export async function settingsDefaultChosen(name: string | null): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_default_chosen', { name });
}

export async function settingsProtonRemove(index: number): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_proton_remove', { index });
}

export async function settingsBuildChosen(index: number): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_build_chosen', { index });
}

export async function settingsProtonRefresh(): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_proton_refresh');
}

export async function settingsProtonShowOlder(): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_proton_show_older');
}

export async function settingsReleaseDownload(index: number): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_release_download', { index });
}

export async function settingsProtonStop(): Promise<SettingsPage> {
	return invoke<SettingsPage>('settings_proton_stop');
}

// The Accounts page.
export async function accountsGet(): Promise<AccountsPage> {
	return invoke<AccountsPage>('accounts_get');
}

export async function accountsCheck(): Promise<AccountsPage> {
	return invoke<AccountsPage>('accounts_check');
}

export async function accountsStart(index: number): Promise<AccountsPage> {
	return invoke<AccountsPage>('accounts_start', { index });
}

export async function accountsSignOut(index: number): Promise<AccountsPage> {
	return invoke<AccountsPage>('accounts_sign_out', { index });
}

export async function accountsCancel(): Promise<AccountsPage> {
	return invoke<AccountsPage>('accounts_cancel');
}

export async function accountsUseBrowser(): Promise<AccountsPage> {
	return invoke<AccountsPage>('accounts_use_browser');
}

export async function accountsCodeEdited(text: string): Promise<AccountsPage> {
	return invoke<AccountsPage>('accounts_code_edited', { text });
}

export async function accountsFinish(): Promise<AccountsPage> {
	return invoke<AccountsPage>('accounts_finish');
}

export async function accountsSteamSignIn(): Promise<AccountsPage> {
	return invoke<AccountsPage>('accounts_steam_sign_in');
}

export async function accountsSteamCancel(): Promise<AccountsPage> {
	return invoke<AccountsPage>('accounts_steam_cancel');
}

export async function accountsSteamSignOut(): Promise<AccountsPage> {
	return invoke<AccountsPage>('accounts_steam_sign_out');
}

export async function accountsXboxOpen(): Promise<AccountsPage> {
	return invoke<AccountsPage>('accounts_xbox_open');
}

export async function accountsSetGogPlaytime(on: boolean): Promise<AccountsPage> {
	return invoke<AccountsPage>('accounts_set_gog_playtime', { on });
}

export async function accountsSetGogGalaxy(on: boolean): Promise<AccountsPage> {
	return invoke<AccountsPage>('accounts_set_gog_galaxy', { on });
}

// The Collections sheet.
export async function collectionsOpen(marked: boolean): Promise<CollectionsPage> {
	return invoke<CollectionsPage>('collections_open', { marked });
}

export async function collectionsClose(): Promise<CollectionsPage> {
	return invoke<CollectionsPage>('collections_close');
}

export async function collectionsGet(): Promise<CollectionsPage> {
	return invoke<CollectionsPage>('collections_get');
}

export async function collectionsDraft(text: string): Promise<CollectionsPage> {
	return invoke<CollectionsPage>('collections_draft', { text });
}

export async function collectionsRenameDraft(text: string): Promise<CollectionsPage> {
	return invoke<CollectionsPage>('collections_rename_draft', { text });
}

export async function collectionsCreate(): Promise<CollectionsPage> {
	return invoke<CollectionsPage>('collections_create');
}

export async function collectionsSaveView(): Promise<CollectionsPage> {
	return invoke<CollectionsPage>('collections_save_view');
}

export async function collectionsRename(name: string): Promise<CollectionsPage> {
	return invoke<CollectionsPage>('collections_rename', { name });
}

export async function collectionsRenameDone(): Promise<CollectionsPage> {
	return invoke<CollectionsPage>('collections_rename_done');
}

export async function collectionsSetMember(
	name: string,
	member: boolean
): Promise<CollectionsPage> {
	return invoke<CollectionsPage>('collections_set_member', { name, member });
}

export async function collectionsDelete(name: string): Promise<CollectionsPage> {
	return invoke<CollectionsPage>('collections_delete', { name });
}

export async function collectionsApply(name: string): Promise<CollectionsPage> {
	return invoke<CollectionsPage>('collections_apply', { name });
}

// The Store page: the games the signed-in stores say the user owns.
export type StoreChip = { key: string; label: string; count: number | null; chosen: boolean };
export type StorePage = {
	ready: boolean;
	cards: Card[];
	chips: { sources: StoreChip[]; sorts: StoreChip[] };
	cursor: number;
	empty: { title: string; accounts: boolean } | null;
	search: string;
	status: string | null;
};

// A store game's page.
export type OwnedPage = {
	id: string;
	title: string;
	source_line: string;
	cover: string | null;
	installed: boolean;
	details: [string, string][];
	// 0 installed (Open in library), 1 Install and Import, 2 queued,
	// 3 running, 4 paused.
	job: number;
	// What the running button says, such as "Installing…".
	job_label: string;
	// How far the install is, 0 to 100; -1 for no bar.
	progress: number;
	// "Also install its DLC (n)", empty when there is none to give.
	dlc: string;
	dlc_on: boolean;
	// What installing it would do, or why it cannot be.
	note: string;
};

export async function storeGet(): Promise<StorePage> {
	return invoke<StorePage>('store_get');
}

export async function storePick(key: string): Promise<StorePage> {
	return invoke<StorePage>('store_pick', { key });
}

export async function storeSearch(text: string): Promise<StorePage> {
	return invoke<StorePage>('store_search', { text });
}

export async function storeCycleSort(): Promise<StorePage> {
	return invoke<StorePage>('store_cycle_sort');
}

export async function storeClick(id: string): Promise<StorePage> {
	return invoke<StorePage>('store_click', { id });
}

export async function storeStep(step: string): Promise<StorePage> {
	return invoke<StorePage>('store_step', { step });
}

export async function storeFit(columns: number, height: number): Promise<StorePage> {
	return invoke<StorePage>('store_fit', { columns, height });
}

export async function storeBack(): Promise<StorePage> {
	return invoke<StorePage>('store_back');
}

export async function storeRefresh(): Promise<StorePage> {
	return invoke<StorePage>('store_refresh');
}

// A store was signed in or out: the lists are what the accounts say now.
export async function storeAccountsChanged(): Promise<StorePage> {
	return invoke<StorePage>('store_accounts_changed');
}

export async function ownedGet(id: string): Promise<OwnedPage> {
	return invoke<OwnedPage>('owned_get', { id });
}

// Install: queues the game where Settings says, with the DLC unless the box
// was unticked.
export async function ownedInstall(id: string): Promise<OwnedPage> {
	return invoke<OwnedPage>('owned_install', { id });
}

// Import: the folder dialog, then the game is taken from the folder chosen.
export async function ownedImport(id: string): Promise<void> {
	return invoke<void>('owned_import', { id });
}

export async function ownedResume(id: string): Promise<OwnedPage> {
	return invoke<OwnedPage>('owned_resume', { id });
}

export async function ownedToggleDlc(id: string): Promise<OwnedPage> {
	return invoke<OwnedPage>('owned_toggle_dlc', { id });
}

// Stop waiting for Steam to start a Steam game's install.
export async function ownedForget(id: string): Promise<OwnedPage> {
	return invoke<OwnedPage>('owned_forget', { id });
}

// A job in the Downloads page's lists. Every text is Rust's; an empty one
// is not drawn.
export type JobRow = {
	id: string;
	title: string;
	// "Install · Epic".
	line: string;
	// A dim line: "Waits until the game is closed".
	note: string;
	// Why it failed, in red.
	error: string;
	// How a finished job ended.
	verdict: string;
	verdict_bad: boolean;
	// The first button (Pause, Resume, Try again), Cancel and Open; an empty
	// label is no button.
	first: string;
	first_primary: boolean;
	cancel: string;
	open: string;
};

// The Downloads page: what the queue is doing, waiting and has finished.
export type DownloadsPage = {
	// Nothing queued and nothing finished: no tiles.
	empty: boolean;
	// Nothing at all going on: the page invites to the Store.
	invite: boolean;
	// How many jobs wait, the running one included.
	count: number;
	counts: { up_next: string; paused: string; finished: string };
	// Closing was asked for while a job runs.
	asking: boolean;
	running: {
		id: string;
		title: string;
		line: string;
		// How far, 0 to 100, or -1 when the engine did not say.
		percent: number;
		progress_line: string;
		pause_label: string;
		cancel_label: string;
		pause_on: boolean;
		cancel_on: boolean;
	} | null;
	up_next: JobRow[];
	paused: JobRow[];
	finished: JobRow[];
	// What the queue last said.
	message: string | null;
	// The running job's line, or how many jobs wait paused.
	summary: string | null;
	// Steam has something going: its card, with Open Steam's downloads.
	steam_show: boolean;
	steam_rows: SteamRow[];
};

// A line of the Steam card: an install, an uninstall or a verify asked of
// Steam, or what Steam is downloading.
export type SteamRow = {
	app: string;
	title: string;
	line: string;
	// How far, 0 to 100, or -1 for no bar.
	percent: number;
	// Stop waiting, for an install Steam has not started; empty is no button.
	forget: string;
};

export async function downloadsGet(): Promise<DownloadsPage> {
	return invoke<DownloadsPage>('downloads_get');
}

export async function downloadsPause(id: string): Promise<DownloadsPage> {
	return invoke<DownloadsPage>('downloads_pause', { id });
}

export async function downloadsResume(id: string): Promise<DownloadsPage> {
	return invoke<DownloadsPage>('downloads_resume', { id });
}

export async function downloadsCancel(id: string): Promise<DownloadsPage> {
	return invoke<DownloadsPage>('downloads_cancel', { id });
}

export async function downloadsClear(): Promise<DownloadsPage> {
	return invoke<DownloadsPage>('downloads_clear');
}

// Where a finished job's game is to open: its page in the library, or in the
// Store when the scan has it not.
export type OpenAt = { kind: 'game' | 'store'; id: string };

export async function downloadsOpen(id: string): Promise<OpenAt | null> {
	return invoke<OpenAt | null>('downloads_open', { id });
}

// Close anyway: the engine stops, the queue is held for next time, and the
// window goes once it has.
export async function downloadsCloseAnyway(): Promise<DownloadsPage> {
	return invoke<DownloadsPage>('downloads_close_anyway');
}

// Keep downloading, from the question about closing.
export async function downloadsStayOpen(): Promise<DownloadsPage> {
	return invoke<DownloadsPage>('downloads_stay_open');
}

// Stop waiting for Steam to start an install it was asked for.
export async function steamStopWaiting(app: string): Promise<DownloadsPage> {
	return invoke<DownloadsPage>('steam_stop_waiting', { app });
}

// Opens Steam's own downloads page.
export async function steamOpenDownloads(): Promise<void> {
	return invoke<void>('steam_open_downloads');
}
