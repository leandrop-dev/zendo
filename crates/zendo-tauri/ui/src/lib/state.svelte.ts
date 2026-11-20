// What the shell knows: the words and colours Rust sent, and the events Rust
// pushes (the colour scheme, the controller).

import { listen } from '@tauri-apps/api/event';
import { bootstrap, inTauri, toggleFullscreen, type Bootstrap, type Palette } from './api.js';
import { applyPalette, DEFAULT_PALETTE } from './theme.js';
import { gameMode } from './game_mode.svelte.js';
import { library } from './library.svelte.js';

class App {
	/// What `bootstrap` returned; `null` until it does, and in a browser.
	data = $state<Bootstrap | null>(null);
	/// Why the shell could not start, if it could not.
	error = $state<string | null>(null);
	/// A line said at the foot for a moment: a Nexus download, a key.
	notice = $state('');

	/// A message in the window's language: the bundle from Rust, or the id
	/// itself while there is none (a browser preview).
	t(id: string): string {
		return this.data?.messages[id] ?? id;
	}

	/// Starts the shell. In a browser the IPC is not there: the preview keeps
	/// Zendo's own colours and shows message ids.
	async start(): Promise<void> {
		if (!inTauri()) {
			applyPalette(DEFAULT_PALETTE);
			return;
		}
		try {
			this.data = await bootstrap();
			applyPalette(this.data.palette);
		} catch (error) {
			this.error = `The shell could not start: ${String(error)}`;
			return;
		}
		await listen<{ language: string; messages: Record<string, string> }>(
			'language_changed',
			(event) => {
				if (this.data) {
					this.data.language = event.payload.language;
					this.data.messages = event.payload.messages;
				}
			}
		);
		await listen<Palette>('theme_changed', (event) => {
			if (this.data) {
				this.data.palette = event.payload;
			}
			applyPalette(event.payload);
		});
		await listen<string>('pad', (event) => {
			void this.pad(event.payload);
		});
	}

	/// Full screen on or off, as F11 and the pad ask.
	async toggleFullscreen(): Promise<void> {
		if (!inTauri()) {
			return;
		}
		const fullscreen = await toggleFullscreen();
		if (this.data) {
			this.data.fullscreen = fullscreen;
		}
	}

	/// What a controller asks for, as the same moves the keyboard makes. The
	/// Game Mode screen takes them all while it shows.
	async pad(move: string): Promise<void> {
		if (move === 'toggle-game-mode') {
			gameMode.toggle();
			return;
		}
		if (gameMode.screen) {
			gameMode.pad(move);
			return;
		}
		switch (move) {
			case 'left':
			case 'right':
			case 'up':
			case 'down':
				await library.step(move);
				break;
			case 'sort':
				await library.cycleSort();
				break;
			case 'show-hidden':
				await library.showHidden();
				break;
			case 'back':
				await library.back();
				break;
			default:
				break;
		}
	}
}

// The shell's one store.
export const app = new App();
