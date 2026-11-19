<script lang="ts">
	import { convertFileSrc } from '@tauri-apps/api/core';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import { inTauri, ownedGet, type OwnedPage } from '#lib/api.js';
	import { app } from '#lib/state.svelte.js';

	// A store game's page: what the store says of it. Install and the queue
	// are the Downloads part of the slice.
	let id = $derived(page.params.id ?? '');
	let owned = $state<OwnedPage | null>(null);
	let error = $state<string | null>(null);
	let cover = $derived(owned?.cover ? convertFileSrc(owned.cover) : null);

	onMount(async () => {
		if (!inTauri() || !id) {
			return;
		}
		try {
			owned = await ownedGet(id);
		} catch (err) {
			error = String(err);
		}
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
