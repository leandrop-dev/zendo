<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { listen } from '@tauri-apps/api/event';
	import { onMount } from 'svelte';
	import Chips from '#lib/components/library/chips.svelte';
	import CollectionsSheet from '#lib/components/collections-sheet.svelte';
	import LocalForm from '#lib/components/local-form.svelte';
	import Grid from '#lib/components/library/grid.svelte';
	import { collectionsApply, collectionsGet, inTauri, type CollectionsPage } from '#lib/api.js';
	import { library } from '#lib/library.svelte.js';
	import { app } from '#lib/state.svelte.js';

	// The grid's keys: the arrows and the pages move the cursor, and Escape
	// takes back the marks and then the search, as the Slint window does.
	const STEPS: Record<string, string> = {
		ArrowLeft: 'left',
		ArrowRight: 'right',
		ArrowUp: 'up',
		ArrowDown: 'down',
		PageUp: 'page-up',
		PageDown: 'page-down',
		Home: 'first',
		End: 'last'
	};

	function key(event: KeyboardEvent) {
		if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) {
			return;
		}
		const step = STEPS[event.key];
		if (step) {
			event.preventDefault();
			void library.step(step);
		} else if (event.key === 'Escape') {
			event.preventDefault();
			void library.back();
		} else if (event.key === 'Enter') {
			const id = library.current?.id;
			if (id) {
				event.preventDefault();
				void goto(resolve('/game/[id]', { id }));
			}
		}
	}

	// The Add game sheet, the Library's own; the game page opens it to edit.
	let adding = $state(false);

	// The Collections sheet.
	let collections = $state<CollectionsPage | null>(null);

	async function loadCollections(): Promise<void> {
		if (!inTauri()) {
			return;
		}
		collections = await collectionsGet();
	}

	function collectionAct(action: () => Promise<CollectionsPage>): void {
		void action().then((page) => {
			collections = page;
		});
	}

	async function applyView(name: string): Promise<void> {
		collections = await collectionsApply(name);
		await library.pick(`saved:${name}`);
	}

	onMount(() => {
		window.addEventListener('keydown', key);
		const changed = listen('collections_changed', () => void loadCollections());
		return () => {
			window.removeEventListener('keydown', key);
			void changed.then((stop) => stop());
		};
	});
</script>

<div class="flex h-full min-h-0 flex-col">
	<div class="flex flex-wrap items-center gap-2 px-6 pt-6 pb-1">
		<Chips chips={library.chips.sources} />
		<div class="flex-1"></div>
		<button
			type="button"
			onclick={() => (adding = true)}
			class="border border-line px-3 py-1 text-[13px] text-foreground uppercase hover:bg-raised"
		>
			{app.t('library-add-game')}
		</button>
		<Chips chips={library.chips.sorts} />
	</div>
	{#if library.chips.organize.length > 0}
		<div class="px-6 pb-1">
			<Chips chips={library.chips.organize} />
		</div>
	{/if}

	{#if library.chips.marked}
		<div class="flex flex-wrap items-center gap-2 px-6 pb-1">
			<span class="text-xs text-dim uppercase">{library.chips.marked}</span>
			<Chips chips={library.chips.bulk} />
		</div>
	{/if}

	<div class="min-h-0 flex-1">
		<Grid />
	</div>

	{#if library.status}
		<p class="border-t border-line px-6 py-2 text-sm text-dim">{library.status}</p>
	{/if}

	{#if adding}
		<LocalForm
			onclose={() => (adding = false)}
			onsaved={(id) => {
				adding = false;
				void goto(resolve('/game/[id]', { id }));
			}}
		/>
	{/if}

	{#if collections?.open}
		<CollectionsSheet {collections} act={collectionAct} onapply={(name) => void applyView(name)} />
	{/if}
</div>
