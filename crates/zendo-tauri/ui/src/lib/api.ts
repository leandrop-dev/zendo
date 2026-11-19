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

// The Settings page: General, Library and Diagnostics.
export type SettingsPage = {
	languages: { tag: string | null; label: string; chosen: boolean }[];
	themes: { tag: string; label: string; chosen: boolean }[];
	theme_about: string;
	theme_note: SettingsRow | null;
	theme_problems: string[];
	tray: boolean;
	covers: boolean;
	// Downloads can start now: the setting is on and nothing runs.
	covers_now: boolean;
	auto_update: boolean;
	workers: { count: number | null; label: string; chosen: boolean }[];
	sources: SettingsRow[];
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
