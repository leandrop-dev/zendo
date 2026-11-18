// The Library page's state in the frontend: what Rust sent, and the commands
// that change it. The rules (what a filter lets through, the chips' counts,
// the cursor) are `zendo_shell::library::Browser`'s; nothing is recomputed
// here.

import { listen } from '@tauri-apps/api/event';
import {
	inTauri,
	libraryBack,
	libraryClick,
	libraryCycleSort,
	libraryFit,
	libraryList,
	libraryPick,
	librarySearch,
	libraryShowHidden,
	libraryStep,
	libraryToggleMark,
	type Card,
	type Chips,
	type Empty,
	type LibraryFiltered,
	type LibraryList
} from './api.js';

/// How long typing waits before the search is asked for.
const SEARCH_DELAY = 120;

class Library {
	ready = $state(false);
	/// Every game's card, by id: sent once, updated by the events to come.
	cards = $state<Record<string, Card>>({});
	/// The ids the filter shows, in the sort's order.
	shown = $state<string[]>([]);
	chips = $state<Chips>({
		views: [],
		sources: [],
		sorts: [],
		organize: [],
		bulk: [],
		marked: null
	});
	cursor = $state(0);
	empty = $state<Empty | null>(null);
	status = $state<string | null>(null);
	search = $state('');
	marked = $state<string[]>([]);

	#timer: ReturnType<typeof setTimeout> | undefined;

	/// The game under the cursor.
	get current(): Card | null {
		const id = this.shown[this.cursor];
		return id === undefined ? null : (this.cards[id] ?? null);
	}

	/// Starts the page: the list once the scan has told it is ready.
	async start(): Promise<void> {
		if (!inTauri()) {
			return;
		}
		await listen('library_loaded', () => {
			void this.list();
		});
		// A game's page changed a mark or a count: the cards and chips follow.
		await listen('library_changed', () => {
			void this.list();
		});
		await listen<{ id: string; path: string }>('cover_changed', (event) => {
			const card = this.cards[event.payload.id];
			if (card) {
				this.cards = {
					...this.cards,
					[card.id]: { ...card, cover: event.payload.path }
				};
			}
		});
		await listen<string>('library_status', (event) => {
			this.status = event.payload;
		});
		await this.list();
	}

	async list(): Promise<void> {
		try {
			this.apply(await libraryList());
		} catch (error) {
			this.status = String(error);
		}
	}

	private apply(list: LibraryList): void {
		this.ready = list.ready;
		if (list.cards.length > 0) {
			this.cards = Object.fromEntries(list.cards.map((card) => [card.id, card]));
		}
		this.use(list);
	}

	private use(list: LibraryFiltered): void {
		this.shown = list.shown;
		this.chips = list.chips;
		this.cursor = list.cursor;
		this.empty = list.empty;
		this.status = list.status;
		this.search = list.search;
		this.marked = list.marked;
	}

	private async run(action: () => Promise<LibraryFiltered>): Promise<void> {
		if (!inTauri()) {
			return;
		}
		try {
			this.use(await action());
		} catch (error) {
			this.status = String(error);
		}
	}

	async pick(key: string): Promise<void> {
		await this.run(() => libraryPick(key));
	}

	/// Typing waits a moment, so a word is one search, not one a letter.
	setSearch(text: string): void {
		this.search = text;
		clearTimeout(this.#timer);
		this.#timer = setTimeout(() => {
			void this.run(() => librarySearch(text));
		}, SEARCH_DELAY);
	}

	async click(id: string): Promise<void> {
		await this.run(() => libraryClick(id));
	}

	/// `left`, `right`, `up`, `down`, `page-up`, `page-down`, `first` or `last`.
	async step(step: string): Promise<void> {
		await this.run(() => libraryStep(step));
	}

	async fit(columns: number, height: number): Promise<void> {
		await this.run(() => libraryFit(columns, height));
	}

	async cycleSort(): Promise<void> {
		await this.run(libraryCycleSort);
	}

	async back(): Promise<void> {
		await this.run(libraryBack);
	}

	async showHidden(): Promise<void> {
		await this.run(libraryShowHidden);
	}

	/// Marks a game, or leaves it; no id marks the card under the cursor.
	async toggleMark(id?: string): Promise<void> {
		await this.run(() => libraryToggleMark(id));
	}
}

/// The Library's one store.
export const library = new Library();
