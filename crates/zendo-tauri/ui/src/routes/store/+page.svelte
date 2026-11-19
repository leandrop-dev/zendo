<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { listen } from '@tauri-apps/api/event';
	import { onMount } from 'svelte';
	import Card from '#lib/components/library/card.svelte';
	import {
		inTauri,
		storeClick,
		storeGet,
		storePick,
		storeRefresh,
		storeSearch,
		type StorePage
	} from '#lib/api.js';
	import { app } from '#lib/state.svelte.js';

	// The Store: the games the signed-in stores say the user owns, filtered by
	// the store chips and the search, and a card opens its page.
	let store = $state<StorePage | null>(null);
	let error = $state<string | null>(null);

	async function load(): Promise<void> {
		if (!inTauri()) {
			return;
		}
		try {
			store = await storeGet();
		} catch (err) {
			error = String(err);
		}
	}

	async function act(action: () => Promise<StorePage>): Promise<void> {
		try {
			store = await action();
		} catch (err) {
			error = String(err);
		}
	}

	async function open(id: string): Promise<void> {
		await act(() => storeClick(id));
		await goto(resolve('/store/[id]', { id }));
	}

	onMount(() => {
		void load();
		const changed = listen('store_changed', () => void load());
		return () => {
			void changed.then((stop) => stop());
		};
	});
</script>

<svelte:head>
	<title>Store · Zendo</title>
</svelte:head>

{#if error}
	<p class="p-8 text-sm text-bad">{error}</p>
{:else if !store}
	<p class="p-8 text-sm text-dim">{app.t('library-looking')}</p>
{:else}
	<div class="flex h-full min-h-0 flex-col">
		<div class="flex flex-wrap items-center gap-2 px-6 pt-6 pb-1">
			{#each store.chips.sources as chip (chip.key)}
				<button
					type="button"
					aria-pressed={chip.chosen}
					onclick={() => void act(() => storePick(chip.key))}
					class="border px-3 py-1 text-[13px] uppercase {chip.chosen
						? 'border-accent-mark text-accent-mark'
						: 'border-line text-foreground hover:bg-raised'}"
				>
					{chip.label}
					{#if chip.count !== null}
						<span class="ml-1.5 text-xs {chip.chosen ? 'text-accent-mark' : 'text-dim'}">
							{chip.count}
						</span>
					{/if}
				</button>
			{/each}
			<div class="flex-1"></div>
			<button
				type="button"
				onclick={() => void act(storeRefresh)}
				class="border border-line px-3 py-1 text-[13px] text-foreground uppercase hover:bg-raised"
			>
				{app.t('library-refresh')}
			</button>
			{#each store.chips.sorts as chip (chip.key)}
				<button
					type="button"
					aria-pressed={chip.chosen}
					onclick={() => void act(() => storePick(chip.key))}
					class="border px-3 py-1 text-[13px] uppercase {chip.chosen
						? 'border-accent-mark text-accent-mark'
						: 'border-line text-foreground hover:bg-raised'}"
				>
					{chip.label}
				</button>
			{/each}
		</div>

		<div class="px-6 pb-1">
			<input
				type="search"
				value={store.search}
				oninput={(event) => void act(() => storeSearch(event.currentTarget.value))}
				placeholder={app.t('library-search')}
				class="h-[34px] w-[220px] border border-line bg-surface px-2 text-sm placeholder:text-dim focus:border-accent-mark focus:outline-none"
			/>
		</div>

		<div class="min-h-0 flex-1 overflow-y-auto p-6">
			{#if store.empty}
				<div class="flex h-full flex-col items-center justify-center gap-2 text-center">
					<p class="text-[15px] text-dim uppercase">{store.empty.title}</p>
					{#if store.empty.accounts}
						<button
							type="button"
							onclick={() => void goto(resolve('/accounts'))}
							class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
						>
							{app.t('page-accounts')}
						</button>
					{/if}
				</div>
			{:else}
				<div class="flex flex-wrap gap-2">
					{#each store.cards as card (card.id)}
						<Card
							{card}
							selected={false}
							marked={false}
							onclick={() => void open(card.id)}
							onmark={() => {}}
						/>
					{/each}
				</div>
			{/if}
		</div>

		{#if store.status}
			<p class="border-t border-line px-6 py-2 text-sm text-dim">{store.status}</p>
		{/if}
	</div>
{/if}
