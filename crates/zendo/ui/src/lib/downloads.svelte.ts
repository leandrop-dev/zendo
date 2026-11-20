// The downloads in the frontend: the queue as Rust keeps it, the line at the
// foot of the window, and looking again while a job runs for its progress.
// The rules (one job at a time, pause, cancel, resume) are
// `zendo_shell::queue`'s; nothing is decided here.

import { listen } from '@tauri-apps/api/event';
import {
	downloadsCancel,
	downloadsClear,
	downloadsCloseAnyway,
	downloadsGet,
	downloadsPause,
	downloadsResume,
	downloadsStayOpen,
	inTauri,
	steamStopWaiting,
	type DownloadsPage
} from './api.js';

/// How often a running job's progress is asked for: the engine writes its
/// log many times a second, and the page needs only to keep up.
const POLL = 500;

class Downloads {
	page = $state<DownloadsPage | null>(null);

	#timer: ReturnType<typeof setTimeout> | undefined;

	/// How many jobs wait, for the top bar's mark.
	get count(): number {
		return this.page?.count ?? 0;
	}

	/// The line at the foot of the window: what runs, or how many wait.
	get summary(): string | null {
		return this.page?.summary ?? null;
	}

	/// Starts the page: once now, and whenever the queue changes.
	async start(): Promise<void> {
		if (!inTauri()) {
			return;
		}
		await listen('downloads_changed', () => {
			void this.refresh();
		});
		await this.refresh();
	}

	/// Asks Rust for the queue, and looks again while a job runs.
	async refresh(): Promise<void> {
		if (!inTauri()) {
			return;
		}
		clearTimeout(this.#timer);
		try {
			this.page = await downloadsGet();
		} catch {
			return;
		}
		if (this.page.running) {
			this.#timer = setTimeout(() => void this.refresh(), POLL);
		}
	}

	#use(action: () => Promise<DownloadsPage>): void {
		void action()
			.then((page) => {
				this.page = page;
			})
			.catch(() => undefined);
	}

	pause(id: string): void {
		this.#use(() => downloadsPause(id));
	}

	resume(id: string): void {
		this.#use(() => downloadsResume(id));
	}

	cancel(id: string): void {
		this.#use(() => downloadsCancel(id));
	}

	/// Stop waiting for Steam to start an install it was asked for.
	stopSteamWaiting(app: string): void {
		this.#use(() => steamStopWaiting(app));
	}

	clear(): void {
		this.#use(downloadsClear);
	}

	/// Close anyway: the engine stops, the queue is held, and the window goes.
	closeAnyway(): void {
		this.#use(downloadsCloseAnyway);
	}

	/// Keep downloading.
	stayOpen(): void {
		this.#use(downloadsStayOpen);
	}
}

/// The downloads' one store.
export const downloads = new Downloads();
