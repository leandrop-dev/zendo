// Nexus Mods in the frontend: the sheet that asks which game a link's mod is
// for, and the lines Nexus says at the window's foot. The mods themselves are
// the Mods card's; its downloads are asked for by the game's page.

import { listen } from '@tauri-apps/api/event';
import { inTauri, nexusCancel, nexusChoose, nexusGet, nexusNotice, type NexusPage } from './api.js';
import { app } from './state.svelte.js';

/// How long a line from Nexus stays at the foot.
const NOTICE_MS = 6000;

class Nexus {
	page = $state<NexusPage | null>(null);

	async start(): Promise<void> {
		if (!inTauri()) {
			return;
		}
		await listen('nexus_changed', () => void this.refresh());
		await listen<string>('nexus_said', (event) => this.said(event.payload));
		// A link can arrive before the page listens.
		const kept = await nexusNotice();
		if (kept) {
			this.said(kept);
		}
	}

	/// A line at the foot for a moment.
	said(text: string): void {
		app.notice = text;
		setTimeout(() => {
			if (app.notice === text) {
				app.notice = '';
			}
		}, NOTICE_MS);
	}

	async refresh(): Promise<void> {
		if (!inTauri()) {
			return;
		}
		this.page = await nexusGet(null);
		const kept = await nexusNotice();
		if (kept) {
			this.said(kept);
		}
	}

	/// A game was chosen for the link.
	async choose(at: number): Promise<void> {
		this.page = await nexusChoose(at);
	}

	/// Escape, Cancel and a press outside.
	async cancel(): Promise<void> {
		this.page = await nexusCancel();
	}

	/// The link chooser is over the window.
	get choosing(): boolean {
		return this.page?.choosing ?? false;
	}
}

/// Nexus's one store.
export const nexus = new Nexus();
