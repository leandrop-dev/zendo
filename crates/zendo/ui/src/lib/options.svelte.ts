// The Launch options sheet in the frontend: the form as Rust keeps it, and
// the presses that change it. The form's rules are
// `zendo_shell::launch_options`'s; nothing is decided here.

import { listen } from '@tauri-apps/api/event';
import {
	inTauri,
	optionsBrowse,
	optionsChoose,
	optionsClose,
	optionsDiscard,
	optionsEdit,
	optionsGet,
	optionsKeepEditing,
	optionsOpen,
	optionsSave,
	optionsSync,
	optionsToggle,
	tr,
	type OptionsSheet
} from './api.js';
import { app } from './state.svelte.js';

/// The typed fields, kept apart from the sheet: writing the whole sheet back
/// while the user types would move the cursor.
export type OptionsFields = {
	executable: string;
	language: string;
	width: string;
	height: string;
	frame_cap: string;
	game_width: string;
	game_height: string;
	fps: string;
	more: string;
	arguments: string;
	environment: string;
	before: string;
	after: string;
};

/// The field ids the sheet sends, in the order they are written.
const KEYS: readonly (keyof OptionsFields)[] = [
	'executable',
	'language',
	'width',
	'height',
	'frame_cap',
	'game_width',
	'game_height',
	'fps',
	'more',
	'arguments',
	'environment',
	'before',
	'after'
];

function blank(): OptionsFields {
	return {
		executable: '',
		language: '',
		width: '',
		height: '',
		frame_cap: '',
		game_width: '',
		game_height: '',
		fps: '',
		more: '',
		arguments: '',
		environment: '',
		before: '',
		after: ''
	};
}

function fieldsOf(sheet: OptionsSheet): OptionsFields {
	return {
		executable: sheet.executable,
		language: sheet.language,
		width: sheet.width,
		height: sheet.height,
		frame_cap: sheet.frame_cap,
		game_width: sheet.game_width,
		game_height: sheet.game_height,
		fps: sheet.fps,
		more: sheet.more,
		arguments: sheet.arguments,
		environment: sheet.environment,
		before: sheet.before,
		after: sheet.after
	};
}

class Options {
	sheet = $state<OptionsSheet | null>(null);
	fields = $state<OptionsFields>(blank());

	/// The sheet is on screen.
	get open(): boolean {
		return this.sheet?.open ?? false;
	}

	async start(): Promise<void> {
		if (!inTauri()) {
			return;
		}
		// A sync ended, or the program picked: the sheet is asked again.
		await listen('options_changed', () => {
			if (this.open) {
				void this.refresh();
			}
		});
	}

	async refresh(): Promise<void> {
		if (inTauri()) {
			this.sheet = await optionsGet();
		}
	}

	/// The Launch options button: opens the form for the game.
	async openFor(id: string): Promise<void> {
		if (!inTauri()) {
			return;
		}
		try {
			const sheet = await optionsOpen(id);
			this.show(sheet);
		} catch (error) {
			// The saved options could not be read: say why at the foot.
			app.error = String(error);
		}
	}

	/// A sheet from Rust; the fields follow only when it opens.
	#receive(sheet: OptionsSheet): void {
		const wasOpen = this.open;
		this.sheet = sheet;
		if (sheet.open && !wasOpen) {
			this.fields = fieldsOf(sheet);
		}
	}

	/// The sheet as it was just opened, fields and all.
	show(sheet: OptionsSheet): void {
		this.sheet = sheet;
		if (sheet.open) {
			this.fields = fieldsOf(sheet);
		}
	}

	#use(action: () => Promise<OptionsSheet>): void {
		void action()
			.then((sheet) => this.#receive(sheet))
			.catch(() => undefined);
	}

	toggle(id: number, on: boolean): void {
		this.#use(() => optionsToggle(id, on));
	}

	choose(list: number, index: number): void {
		this.#use(() => optionsChoose(list, index));
	}

	/// A field was typed in: the digits-only ones keep their digits, which are
	/// written back when the typing had others.
	async edit(field: number, text: string): Promise<void> {
		try {
			const answer = await optionsEdit(field, text);
			this.sheet = answer.sheet;
			const key = KEYS[field];
			if (answer.echo !== null && key) {
				this.fields[key] = answer.echo;
			}
		} catch {
			// Nothing typed reaches the form while it is closed.
		}
	}

	sync(which: number): void {
		this.#use(() => optionsSync(which));
	}

	browse(): void {
		this.#use(() => optionsBrowse());
	}

	/// Escape, Cancel and a click outside: closes it, or asks first.
	close(): void {
		this.#use(optionsClose);
	}

	discard(): void {
		this.#use(optionsDiscard);
	}

	keepEditing(): void {
		this.#use(optionsKeepEditing);
	}

	/// The page is left: the sheet goes without asking (the next game opens
	/// its own).
	dismiss(): void {
		if (this.open) {
			this.sheet = null;
			this.fields = blank();
		}
	}

	/// Save: writes the options, closes the sheet and says it did.
	async save(): Promise<boolean> {
		try {
			const answer = await optionsSave();
			this.show(answer.sheet);
			if (answer.saved) {
				app.notice = await tr('options-saved', { title: answer.title });
			}
			return answer.saved;
		} catch {
			return false;
		}
	}
}

/// The sheet's one store.
export const options = new Options();
