<script lang="ts">
	import { onMount } from 'svelte';
	import { localAbout, localGet, localPickProgram, localSave, type LocalDraft } from '#lib/api.js';
	import { app } from '#lib/state.svelte.js';

	// The Add game sheet, and the same sheet to edit one: its program is
	// chosen through Zendo's own file dialog, and `zendo_core::local`
	// checks what is saved.
	let {
		id = null,
		onclose,
		onsaved
	}: {
		id?: string | null;
		onclose: () => void;
		onsaved: (id: string) => void;
	} = $props();

	// The form is made anew each time it opens, so the id it starts with is
	// the one it edits.
	// svelte-ignore state_referenced_locally
	let form = $state<LocalDraft>({ id, title: '', program: '', arguments: '' });
	let editing = $derived(id !== null);
	let about = $state('');
	let error = $state<string | null>(null);
	let picking = $state(false);

	/// The program's name, as the first title guess: no folder, no suffix.
	function titleOf(path: string): string {
		const name = path.split('/').pop() ?? path;
		return name.replace(/\.(exe|sh|x86_64|AppImage)$/i, '');
	}

	async function refreshAbout(): Promise<void> {
		try {
			about = await localAbout(form.program);
		} catch (err) {
			error = String(err);
		}
	}

	async function choose(): Promise<void> {
		picking = true;
		try {
			const path = await localPickProgram();
			if (path !== null) {
				form.program = path;
				if (form.title.trim() === '') {
					form.title = titleOf(path);
				}
				await refreshAbout();
			}
		} catch (err) {
			error = String(err);
		} finally {
			picking = false;
		}
	}

	async function save(): Promise<void> {
		error = null;
		try {
			onsaved(await localSave(form));
		} catch (err) {
			error = String(err);
		}
	}

	function key(event: KeyboardEvent): void {
		if (event.key === 'Escape') {
			event.preventDefault();
			onclose();
		}
	}

	onMount(() => {
		if (id !== null) {
			localGet(id)
				.then((found) => {
					form = found;
					return refreshAbout();
				})
				.catch((err) => {
					error = String(err);
				});
		} else {
			void refreshAbout();
		}
		window.addEventListener('keydown', key);
		return () => window.removeEventListener('keydown', key);
	});
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60" role="presentation">
	<div
		role="dialog"
		aria-modal="true"
		aria-label={editing ? app.t('local-edit-game') : app.t('local-add-game')}
		class="flex w-[540px] max-w-[calc(100%-3rem)] flex-col gap-4 border border-line bg-background p-6"
	>
		<h2 class="text-lg font-bold tracking-[0.15em] uppercase">
			{editing ? app.t('local-edit-game') : app.t('local-add-game')}
		</h2>

		<label class="flex flex-col gap-1 text-[11px] text-dim uppercase">
			{app.t('local-title')}
			<input
				bind:value={form.title}
				placeholder={app.t('local-title-placeholder')}
				class="border-line bg-surface px-2 py-1.5 text-sm text-foreground normal-case"
			/>
		</label>

		<label class="flex flex-col gap-1 text-[11px] text-dim uppercase">
			{app.t('local-program')}
			<span class="flex gap-2">
				<input
					readonly
					value={form.program}
					placeholder={app.t('local-program-placeholder')}
					class="min-w-0 flex-1 border-line bg-surface px-2 py-1.5 font-mono text-sm text-foreground normal-case"
				/>
				<button
					type="button"
					onclick={choose}
					disabled={picking}
					class="border border-line px-3 py-1.5 text-[13px] text-foreground uppercase hover:bg-raised disabled:opacity-60"
				>
					{picking ? app.t('choosing') : app.t('choose')}
				</button>
			</span>
		</label>

		<p class="text-xs text-dim">{about}</p>

		<label class="flex flex-col gap-1 text-[11px] text-dim uppercase">
			{app.t('local-arguments')}
			<input
				bind:value={form.arguments}
				placeholder={app.t('local-arguments-placeholder')}
				class="border-line bg-surface px-2 py-1.5 font-mono text-sm text-foreground normal-case"
			/>
		</label>

		<p class="text-xs text-dim">{app.t('local-cover-note')}</p>

		{#if error}
			<p class="text-sm text-bad">{error}</p>
		{/if}

		<div class="flex justify-end gap-2">
			<button
				type="button"
				onclick={onclose}
				class="border border-line px-3 py-1.5 text-[13px] text-foreground uppercase hover:bg-raised"
			>
				{app.t('cancel')}
			</button>
			<button
				type="button"
				onclick={save}
				class="border border-accent-mark bg-accent-mark px-3 py-1.5 text-[13px] font-bold text-background uppercase"
			>
				{editing ? app.t('save') : app.t('add')}
			</button>
		</div>
	</div>
</div>
