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
