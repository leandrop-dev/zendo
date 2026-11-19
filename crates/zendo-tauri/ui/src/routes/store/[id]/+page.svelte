<script lang="ts">
	import { convertFileSrc } from '@tauri-apps/api/core';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { listen } from '@tauri-apps/api/event';
	import { onMount } from 'svelte';
	import {
		inTauri,
		ownedForget,
		ownedGet,
		ownedImport,
		ownedInstall,
		ownedResume,
		ownedToggleDlc,
		type OwnedPage
	} from '#lib/api.js';
	import { app } from '#lib/state.svelte.js';

	// A store game's page: what the store says of it, and what the queue
	// would do with it.
	let id = $derived(page.params.id ?? '');
	let owned = $state<OwnedPage | null>(null);
	let error = $state<string | null>(null);
	let cover = $derived(owned?.cover ? convertFileSrc(owned.cover) : null);

	async function load(): Promise<void> {
		if (!inTauri() || !id) {
			return;
		}
		try {
			owned = await ownedGet(id);
		} catch (err) {
			error = String(err);
		}
	}

	async function act(action: () => Promise<OwnedPage>): Promise<void> {
		try {
			owned = await action();
		} catch (err) {
			error = String(err);
		}
	}

	onMount(() => {
		void load();
		const changed = listen('store_changed', () => void load());
		const queued = listen('downloads_changed', () => void load());
		const steam = listen('steam_changed', () => void load());
		return () => {
			void changed.then((stop) => stop());
			void queued.then((stop) => stop());
			void steam.then((stop) => stop());
		};
	});
</script>

<svelte:head>
	<title>{owned?.title ?? 'Store'} · Zendo</title>
</svelte:head>

{#if error}
	<p class="p-8 text-sm text-bad">{error}</p>
{:else if !owned}
	<p class="p-8 text-sm text-dim">{app.t('library-looking')}</p>
{:else}
	{@const game = owned}
	<div class="h-full overflow-y-auto p-6">
		<div class="mx-auto flex max-w-[960px] flex-col gap-4">
			<button
				type="button"
				onclick={() => void goto(resolve('/store'))}
				class="self-start border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
			>
				{app.t('store-back')}
			</button>

			<div class="flex items-start gap-4">
				<div class="h-[180px] w-[128px] shrink-0 overflow-hidden bg-surface">
					{#if cover}
						<img src={cover} alt="" class="h-full w-full object-cover" />
					{:else}
						<span class="flex h-full w-full items-center justify-center p-2 text-center text-sm">
							{owned.title}
						</span>
					{/if}
				</div>
				<div class="min-w-0 flex-1">
					<h1 class="text-[24px] font-bold">{owned.title}</h1>
					<p class="text-[13px] text-dim">{owned.source_line}</p>
				</div>
			</div>

			<div class="flex flex-wrap items-center gap-3">
				{#if owned.installed}
					<button
						type="button"
						onclick={() => void goto(resolve('/game/[id]', { id: game.id }))}
						class="border border-accent-mark px-3 py-1 text-[13px] text-accent-mark uppercase hover:bg-raised"
					>
						{app.t('store-open-in-library')}
					</button>
				{/if}
				{#if owned.job === 1}
					<button
						type="button"
						onclick={() => void act(() => ownedInstall(game.id))}
						class="border border-accent-mark px-3 py-1 text-[13px] text-accent-mark uppercase hover:bg-raised"
					>
						{app.t('install')}
					</button>
					<button
						type="button"
						onclick={() => void ownedImport(game.id)}
						class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
					>
						{app.t('downloads-import')}
					</button>
				{/if}
				{#if owned.job === 2}
					<button
						type="button"
						disabled
						class="border border-accent-mark px-3 py-1 text-[13px] text-accent-mark uppercase opacity-50"
					>
						{app.t('queued')}
					</button>
				{/if}
				{#if owned.job === 3}
					<button
						type="button"
						disabled
						class="border border-accent-mark px-3 py-1 text-[13px] text-accent-mark uppercase opacity-50"
					>
						{owned.job_label}
					</button>
				{/if}
				{#if owned.job === 4}
					<button
						type="button"
						onclick={() => void act(() => ownedResume(game.id))}
						class="border border-accent-mark px-3 py-1 text-[13px] text-accent-mark uppercase hover:bg-raised"
					>
						{app.t('resume')}
					</button>
				{/if}
				{#if owned.job >= 5}
					<button
						type="button"
						disabled={owned.job === 7}
						onclick={() => void act(() => ownedInstall(game.id))}
						class="border border-accent-mark px-3 py-1 text-[13px] text-accent-mark uppercase hover:bg-raised disabled:opacity-50"
					>
						{owned.job_label}
					</button>
				{/if}
				{#if owned.job === 6}
					<button
						type="button"
						onclick={() => void act(() => ownedForget(game.id))}
						class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
					>
						{app.t('steam-stop-waiting')}
					</button>
				{/if}
				{#if owned.job >= 2 && owned.job !== 5}
					<button
						type="button"
						onclick={() => void goto(resolve('/downloads'))}
						class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
					>
						{app.t('see-downloads')}
					</button>
				{/if}
				{#if owned.note}
					<p class="min-w-0 flex-1 text-sm text-dim">{owned.note}</p>
				{/if}
			</div>

			{#if owned.progress >= 0}
				<div class="h-2 bg-raised">
					<div class="h-full bg-accent-mark" style="width: {Math.min(owned.progress, 100)}%"></div>
				</div>
			{/if}

			{#if owned.dlc}
				<label class="flex items-center gap-2 text-sm">
					<input
						type="checkbox"
						checked={owned.dlc_on}
						onchange={() => void act(() => ownedToggleDlc(game.id))}
					/>
					{owned.dlc}
				</label>
			{/if}

			<section class="border border-line bg-surface p-4">
				<dl class="flex flex-col gap-1">
					{#each owned.details as [label, value] (label)}
						<div class="flex gap-4 text-[13px]">
							<dt class="w-[160px] shrink-0 text-dim">{label}</dt>
							<dd class="min-w-0 flex-1 break-words">{value}</dd>
						</div>
					{/each}
				</dl>
			</section>
		</div>
	</div>
{/if}
