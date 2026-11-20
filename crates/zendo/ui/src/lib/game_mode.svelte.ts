// Game Mode in the frontend: the screen's state as Rust keeps it, and the
// moves that change it. The rules (the shelves, the hero, the Start menu) are
// `zendo_shell::game_mode`'s; nothing is decided here.

import { goto } from '$app/navigation';
import { resolve } from '$app/paths';
import { listen } from '@tauri-apps/api/event';
import {
	gameModeBack,
	gameModeBrowse,
	gameModeChoose,
	gameModeCycleShelf,
	gameModeDetails,
	gameModeGet,
	gameModeMenuStep,
	gameModeOpenMenu,
	gameModePick,
	gameModePlay,
	gameModeShelf,
	gameModeShow,
	gameModeSized,
	gameModeStart,
	gameModeStep,
	gameModeToggle,
	inTauri,
	type GameModePage
} from './api.js';
import { app } from './state.svelte.js';

class GameMode {
	page = $state<GameModePage | null>(null);

	/// Game Mode is on.
	get on(): boolean {
		return this.page?.on ?? false;
	}

	/// The screen is what shows: on, and the Library's own grid was not asked
	/// for.
	get screen(): boolean {
		return this.on && !(this.page?.browsing ?? false);
	}

	async start(): Promise<void> {
		if (!inTauri()) {
			return;
		}
		await listen('game_mode_changed', () => void this.refresh());
		this.page = await gameModeGet();
		// Started with --game-mode: the window is full screen already.
		if (app.data?.fullscreen) {
			this.page = await gameModeStart();
		}
	}

	async refresh(): Promise<void> {
		if (inTauri()) {
			this.page = await gameModeGet();
		}
	}

	#use(action: () => Promise<GameModePage>): void {
		void action()
			.then((page) => {
				this.page = page;
			})
			.catch(() => undefined);
	}

	toggle(): void {
		this.#use(gameModeToggle);
	}

	sized(width: number): void {
		if (this.on) {
			this.#use(() => gameModeSized(width));
		}
	}

	shelf(at: number): void {
		this.#use(() => gameModeShelf(at));
	}

	pick(at: number): void {
		this.#use(() => gameModePick(at));
	}

	step(by: number): void {
		this.#use(() => gameModeStep(by));
	}

	cycleShelf(by: number): void {
		this.#use(() => gameModeCycleShelf(by));
	}

	openMenu(): void {
		this.#use(gameModeOpenMenu);
	}

	/// Escape on the screen: the Start menu opens, or closes.
	back(): void {
		this.#use(gameModeBack);
	}

	menuStep(step: string): void {
		this.#use(() => gameModeMenuStep(step));
	}

	choose(at: number): void {
		void gameModeChoose(at).then((answer) => {
			this.page = answer.page;
			if (answer.leave) {
				void goto(resolve('/'));
			}
		});
	}

	/// A or Enter on the screen: the Start menu's choice, or the hero played.
	activate(): void {
		const chosen = this.page?.menu.findIndex((item) => item.chosen) ?? -1;
		if (this.page?.menu_open && chosen >= 0) {
			this.choose(chosen);
		} else {
			this.#use(gameModePlay);
		}
	}

	/// The hero's own page; Escape there comes back to the screen.
	details(): void {
		void gameModeDetails().then((id) => {
			if (id) {
				void goto(resolve('/game/[id]', { id }));
			}
		});
	}

	/// Library in the Start menu: the Library's own grid.
	browse(): void {
		this.#use(() =>
			gameModeBrowse().then(async (page) => {
				await goto(resolve('/'));
				return page;
			})
		);
	}

	/// The screen again, after the Library's grid.
	show(): void {
		this.#use(() =>
			gameModeShow().then(async (page) => {
				await goto(resolve('/'));
				return page;
			})
		);
	}

	/// A pad's move: the screen takes them all while it shows.
	pad(move: string): void {
		if (!this.screen) {
			return;
		}
		const menu = this.page?.menu_open ?? false;
		switch (move) {
			case 'left':
			case 'up':
				if (menu) {
					this.menuStep(move);
				} else {
					this.step(-1);
				}
				break;
			case 'right':
			case 'down':
				if (menu) {
					this.menuStep(move);
				} else {
					this.step(1);
				}
				break;
			case 'activate':
				this.activate();
				break;
			case 'back':
				this.back();
				break;
			case 'sort':
			case 'next-shelf':
				this.cycleShelf(1);
				break;
			case 'prev-shelf':
				this.cycleShelf(-1);
				break;
			default:
				break;
		}
	}

	/// A key on the screen: the arrows, Enter, Escape and Tab. Says whether
	/// it was taken.
	key(event: KeyboardEvent): boolean {
		if (!this.screen) {
			return false;
		}
		const menu = this.page?.menu_open ?? false;
		switch (event.key) {
			case 'ArrowLeft':
				event.preventDefault();
				if (menu) {
					this.menuStep('left');
				} else {
					this.step(-1);
				}
				return true;
			case 'ArrowRight':
				event.preventDefault();
				if (menu) {
					this.menuStep('right');
				} else {
					this.step(1);
				}
				return true;
			case 'ArrowUp':
			case 'ArrowDown':
				if (menu) {
					event.preventDefault();
					this.menuStep(event.key === 'ArrowUp' ? 'up' : 'down');
					return true;
				}
				return false;
			case 'Enter':
				event.preventDefault();
				this.activate();
				return true;
			case 'Escape':
				event.preventDefault();
				this.back();
				return true;
			case 'Tab':
				event.preventDefault();
				this.cycleShelf(1);
				return true;
			default:
				return false;
		}
	}
}

/// Game Mode's one store.
export const gameMode = new GameMode();
