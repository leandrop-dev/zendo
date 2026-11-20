// The FOMOD installer sheet in the frontend: the sheet as Rust keeps it, and
// the presses that change it. Its rules are `zendo_shell::installer`'s,
// kept in `zendo-shell`; nothing is decided here. A sheet can come up
// over any page, which is what `installer_changed` says.

import { listen } from '@tauri-apps/api/event';
import {
	inTauri,
	installerActivate,
	installerBack,
	installerCancel,
	installerFocus,
	installerGet,
	installerNext,
	installerPick,
	type InstallerSheet
} from './api.js';
import { app } from './state.svelte.js';

/// How long a line from the sheet stays at the foot.
const NOTICE_MS = 6000;

class Installer {
	sheet = $state<InstallerSheet | null>(null);

	/// The sheet is over the window.
	get open(): boolean {
		return this.sheet?.open ?? false;
	}

	async start(): Promise<void> {
		if (!inTauri()) {
			return;
		}
		await listen('installer_changed', () => void this.refresh());
		await listen<string>('installer_said', (event) => this.said(event.payload));
		// The sheet's lines are built in Rust: a language change asks for them
		// again.
		await listen('language_changed', () => {
			if (this.open) {
				void this.refresh();
			}
		});
	}

	async refresh(): Promise<void> {
		if (inTauri()) {
			this.sheet = await installerGet();
		}
	}

	/// A line at the foot for a moment: the options are to be chosen, or the
	/// mod was not installed.
	said(text: string): void {
		app.notice = text;
		setTimeout(() => {
			if (app.notice === text) {
				app.notice = '';
			}
		}, NOTICE_MS);
	}

	#use(action: () => Promise<InstallerSheet>): void {
		void action()
			.then((sheet) => {
				this.sheet = sheet;
			})
			.catch(() => undefined);
	}

	/// A radio asks to be on; a box flips.
	pick(option: {
		group: number;
		option: number;
		single: boolean;
		on: boolean;
		enabled: boolean;
	}): void {
		if (!option.enabled) {
			return;
		}
		const on = option.single ? true : !option.on;
		this.#use(() => installerPick(option.group, option.option, on));
	}

	focus(delta: number): void {
		this.#use(() => installerFocus(delta));
	}

	next(): void {
		this.#use(installerNext);
	}

	back(): void {
		this.#use(installerBack);
	}

	activate(): void {
		this.#use(installerActivate);
	}

	/// Escape and Cancel: the sheet closes and the unpacked mod goes.
	cancel(): void {
		this.#use(installerCancel);
	}
}

/// The sheet's one store.
export const installer = new Installer();
