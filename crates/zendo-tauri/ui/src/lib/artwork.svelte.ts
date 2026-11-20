// The art picker in the frontend: the sheet as Rust keeps it, the thumbnails
// as they arrive, and the presses that change it. The picker's rules are
// `zendo_shell::artwork`'s; nothing is decided here.

import { listen } from '@tauri-apps/api/event';
import {
	artAutomatic,
	artChoose,
	artClose,
	artGame,
	artGet,
	artKind,
	artMore,
	artOpen,
	artQuery,
	artSearch,
	inTauri,
	type ArtSheet
} from './api.js';

class Artwork {
	sheet = $state<ArtSheet | null>(null);
	/// The search field's words, kept apart so an answer cannot rewrite them
	/// while the user types.
	query = $state('');

	/// The sheet is on screen.
	get open(): boolean {
		return this.sheet?.open ?? false;
	}

	async start(): Promise<void> {
		if (!inTauri()) {
			return;
		}
		await listen('art_changed', () => void this.refresh());
		await listen<{ image: number; path: string }>('art_thumb', (event) => {
			const thumb = this.sheet?.thumbs.find((one) => one.image === event.payload.image);
			if (thumb) {
				thumb.path = event.payload.path;
			}
		});
	}

	async refresh(): Promise<void> {
		if (inTauri()) {
			this.show(await artGet());
		}
	}

	/// A sheet from Rust; the field follows only when it opens.
	show(sheet: ArtSheet): void {
		const wasOpen = this.open;
		this.sheet = sheet;
		if (sheet.open && !wasOpen) {
			this.query = sheet.query;
		}
	}

	/// Change cover (0) or Change backdrop (1) on the game's page. The key may
	/// be in the keyring, so the sheet opens from Rust's thread and says
	/// `art_changed`.
	async openFor(id: string, kind: number): Promise<void> {
		if (!inTauri()) {
			return;
		}
		const sheet = await artOpen(id, kind);
		// The answer can be the closed one the command took before its thread
		// opened the sheet; a sheet that opened meanwhile stays.
		if (sheet.open || !this.open) {
			this.show(sheet);
		}
	}

	#use(action: () => Promise<ArtSheet>): void {
		void action()
			.then((sheet) => this.show(sheet))
			.catch(() => undefined);
	}

	/// A keystroke in the search field: Rust keeps the words for Enter.
	typed(text: string): void {
		this.query = text;
		void artQuery(text).catch(() => undefined);
	}

	/// Enter and the Search button: the words as typed, then the search.
	async search(): Promise<void> {
		await artQuery(this.query).catch(() => undefined);
		this.#use(artSearch);
	}

	tab(kind: number): void {
		this.#use(() => artKind(kind));
	}

	game(at: number): void {
		this.#use(() => artGame(at));
	}

	choose(image: number): void {
		this.#use(() => artChoose(image));
	}

	more(): void {
		this.#use(artMore);
	}

	/// "Use the automatic one".
	automatic(): void {
		this.#use(artAutomatic);
	}

	/// Escape, Close and a press outside.
	close(): void {
		this.#use(artClose);
	}

	/// The page is left: the sheet goes without asking.
	dismiss(): void {
		if (this.open) {
			this.sheet = null;
			this.query = '';
		}
	}
}

/// The picker's one store.
export const artwork = new Artwork();
