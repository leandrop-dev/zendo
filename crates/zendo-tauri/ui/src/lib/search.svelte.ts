// Search Nexus in the frontend: the sheet as Rust keeps it, the pictures as
// they arrive, and the presses that change it. Its rules are
// `zendo_shell::nexus_search`'s; nothing is decided here.

import { listen } from '@tauri-apps/api/event';
import {
	inTauri,
	searchAdult,
	searchClose,
	searchDownload,
	searchFiles,
	searchGet,
	searchOpen,
	searchOrder,
	searchPage,
	searchRun,
	searchWords,
	type SearchSheet
} from './api.js';

class Search {
	sheet = $state<SearchSheet | null>(null);
	/// The field's words, kept apart while they are typed.
	words = $state('');

	/// The sheet is on screen.
	get open(): boolean {
		return this.sheet?.open ?? false;
	}

	async start(): Promise<void> {
		if (!inTauri()) {
			return;
		}
		await listen('search_changed', () => void this.refresh());
		await listen<{ mod_id: number; path: string }>('search_thumb', (event) => {
			const row = this.sheet?.rows.find((one) => one.mod_id === event.payload.mod_id);
			if (row) {
				row.thumb = event.payload.path;
			}
		});
		// A mod from Nexus's window went in: the marks follow.
		await listen('game_changed', () => {
			if (this.open) {
				void this.refresh();
			}
		});
	}

	async refresh(): Promise<void> {
		if (inTauri()) {
			this.show(await searchGet());
		}
	}

	/// A sheet from Rust; the field follows only when it opens.
	show(sheet: SearchSheet): void {
		const wasOpen = this.open;
		this.sheet = sheet;
		if (sheet.open && !wasOpen) {
			this.words = sheet.words;
		}
	}

	/// Search Nexus on the game's page.
	async openFor(id: string): Promise<void> {
		if (!inTauri()) {
			return;
		}
		const sheet = await searchOpen(id);
		// The answer can be the closed one the command took before its thread
		// opened the sheet; a sheet that opened meanwhile stays.
		if (sheet.open || !this.open) {
			this.show(sheet);
		}
	}

	#use(action: () => Promise<SearchSheet>): void {
		void action()
			.then((sheet) => this.show(sheet))
			.catch(() => undefined);
	}

	/// A keystroke in the field: Rust keeps the words for Enter.
	typed(text: string): void {
		this.words = text;
		void searchWords(text).catch(() => undefined);
	}

	/// Enter and Search, from the start; More asks for the next page.
	async run(more = false): Promise<void> {
		if (!more) {
			await searchWords(this.words).catch(() => undefined);
		}
		this.#use(() => searchRun(more));
	}

	order(at: number): void {
		this.#use(() => searchOrder(at));
	}

	adult(): void {
		this.#use(searchAdult);
	}

	files(mod: number): void {
		this.#use(() => searchFiles(mod));
	}

	page(mod: number, file: number | null): void {
		this.#use(() => searchPage(mod, file));
	}

	download(mod: number, file: number): void {
		this.#use(() => searchDownload(mod, file));
	}

	/// Escape, Close and a press outside.
	close(): void {
		this.#use(searchClose);
	}

	/// The page is left: the sheet goes.
	dismiss(): void {
		if (this.open) {
			this.sheet = null;
			this.words = '';
		}
	}
}

/// The search's one store.
export const search = new Search();
