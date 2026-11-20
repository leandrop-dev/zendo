<script lang="ts">
	import { convertFileSrc } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import { inTauri } from '#lib/api.js';
	import { search } from '#lib/search.svelte.js';
	import { app } from '#lib/state.svelte.js';

	// Search Nexus: a field and the order, the mods found a page at a time
	// with their pictures, and each mod's files when it is opened. Rust holds
	// the search (`zendo_shell::nexus_search`); the rows are flat.

	let sheet = $derived(search.sheet);

	function asset(path: string): string {
		return inTauri() ? convertFileSrc(path) : '';
	}

	function searchNow(): void {
		void search.run(false);
	}

	// Escape closes; Enter in the field searches.
	function key(event: KeyboardEvent): void {
		if (!search.open) {
			return;
		}
		if (event.key === 'Escape') {
			event.preventDefault();
			search.close();
		}
	}

	onMount(() => {
		window.addEventListener('keydown', key);
		return () => window.removeEventListener('keydown', key);
	});
</script>

{#if sheet?.open}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-background/80 p-4">
		<button
			type="button"
			class="absolute inset-0 cursor-default"
			aria-label={app.t('close')}
			onclick={() => search.close()}
		></button>
		<div
			class="relative flex h-[calc(100vh-2rem)] w-[min(980px,100%)] flex-col gap-4 overflow-hidden border border-line bg-background p-6"
		>
			<h2 class="shrink-0 truncate text-lg font-bold uppercase">{sheet.title}</h2>

			<div class="flex shrink-0 gap-3">
				<input
					type="text"
					value={search.words}
					placeholder={app.t('search-placeholder')}
					oninput={(event) => search.typed(event.currentTarget.value)}
					onkeydown={(event) => {
						if (event.key === 'Enter') {
							event.preventDefault();
							searchNow();
						}
					}}
					class="min-w-0 flex-1 border border-line bg-surface px-3 py-1.5 text-sm"
				/>
				<button
					type="button"
					onclick={searchNow}
					class="border border-line px-3 py-1.5 text-[13px] uppercase hover:bg-raised"
				>
					{app.t('art-search')}
				</button>
			</div>

			<div class="flex shrink-0 flex-wrap items-center gap-2">
				{#each sheet.orders as order (order.id)}
					<button
						type="button"
						aria-pressed={order.chosen}
						onclick={() => search.order(order.id)}
						class="border px-3 py-1 text-[13px] {order.chosen
							? 'border-accent-mark text-accent-mark'
							: 'border-line text-foreground hover:bg-raised'}"
					>
						{order.label}
					</button>
				{/each}
				<span class="w-4"></span>
				<button
					type="button"
					aria-pressed={sheet.adult}
					onclick={() => search.adult()}
					class="border px-3 py-1 text-[13px] {sheet.adult
						? 'border-accent-mark text-accent-mark'
						: 'border-line text-foreground hover:bg-raised'}"
				>
					{app.t('search-adult')}
				</button>
			</div>

			{#if sheet.limit}
				<p class="shrink-0 text-[13px] text-dim">{sheet.limit}</p>
			{/if}

			<div class="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto pr-2">
				{#each sheet.rows as row, at (at)}
					{#if row.kind === 0}
						<div class="flex gap-4 border border-line bg-surface p-3">
							<div class="h-[90px] w-[160px] shrink-0 overflow-hidden bg-raised">
								{#if row.thumb}
									<img
										src={asset(row.thumb)}
										alt=""
										loading="lazy"
										class="h-full w-full object-cover"
									/>
								{/if}
							</div>
							<div class="flex min-w-0 flex-1 flex-col gap-1">
								<div class="flex flex-wrap items-center gap-2">
									<p class="truncate text-[16px] font-bold">{row.name}</p>
									{#if row.version}
										<span class="text-[13px] text-dim">{row.version}</span>
									{/if}
									{#if row.have}
										<span class="text-[12px] text-good">{row.have}</span>
									{/if}
									{#if row.adult}
										<span class="text-[12px] text-bad">{row.adult}</span>
									{/if}
								</div>
								<p class="text-[12px] text-dim">{row.byline}</p>
								<p class="text-[13px]">{row.summary}</p>
								<p class="text-[12px] text-dim">{row.numbers}</p>
							</div>
							<div class="flex w-[150px] shrink-0 flex-col gap-2">
								{#if row.files_label}
									<button
										type="button"
										onclick={() => search.files(row.mod_id)}
										class="border border-line px-2 py-1 text-[13px] hover:bg-raised"
									>
										{row.files_label}
									</button>
								{/if}
								<button
									type="button"
									onclick={() => search.page(row.mod_id, null)}
									class="border border-line px-2 py-1 text-[13px] hover:bg-raised"
								>
									{row.page_label}
								</button>
							</div>
						</div>
					{:else if row.kind === 1}
						<div class="flex items-center gap-3 pl-[176px]">
							<p class="min-w-0 flex-1 truncate text-[14px]">{row.name}</p>
							<p class="text-[12px] text-dim">{row.file_facts}</p>
							{#if row.file_have}
								<p class="text-[13px] text-good">{row.file_have}</p>
							{/if}
							{#if row.file_action}
								<button
									type="button"
									onclick={() => search.download(row.mod_id, row.file_id)}
									class="border border-accent-mark bg-accent-mark px-2 py-0.5 text-[12px] font-bold text-background uppercase"
								>
									{row.file_action}
								</button>
							{/if}
						</div>
					{:else}
						<p class="pl-[176px] text-[13px] {row.bad ? 'text-bad' : 'text-dim'}">{row.name}</p>
					{/if}
				{/each}
				{#if sheet.more}
					<button
						type="button"
						disabled={sheet.asking}
						onclick={() => void search.run(true)}
						class="self-start border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-50"
					>
						{app.t('art-show-more')}
					</button>
				{/if}
			</div>

			<div class="flex shrink-0 items-center gap-3 border-t border-line pt-4">
				<p class="min-w-0 flex-1 truncate text-[13px] {sheet.footer_bad ? 'text-bad' : 'text-dim'}">
					{sheet.footer}
				</p>
				<button
					type="button"
					onclick={() => search.close()}
					class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
				>
					{app.t('close')}
				</button>
			</div>
		</div>
	</div>
{/if}
