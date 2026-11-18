// What the shell knows: the words and colours Rust sent, and the events Rust
// pushes (the colour scheme, the controller).

import { listen } from '@tauri-apps/api/event';
import { bootstrap, inTauri, toggleFullscreen, type Bootstrap, type Palette } from './api.js';
import { applyPalette, DEFAULT_PALETTE } from './theme.js';

class App {
	/// What `bootstrap` returned; `null` until it does, and in a browser.
	data = $state<Bootstrap | null>(null);
	/// Why the shell could not start, if it could not.
	error = $state<string | null>(null);

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
	/// grid's moves arrive with the Library (slice 2); Game Mode's with
	/// slice 6.
	async pad(move: string): Promise<void> {
		if (move === 'toggle-game-mode') {
			await this.toggleFullscreen();
		}
	}
}

// The shell's one store.
export const app = new App();
