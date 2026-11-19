<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { onMount } from 'svelte';
	import { downloadsOpen, inTauri, type JobRow } from '#lib/api.js';
	import { downloads } from '#lib/downloads.svelte.js';
	import { app } from '#lib/state.svelte.js';

	// The Downloads page: what the queue is doing, what waits, what is paused
	// and what ended. Rust keeps the queue; this page asks for it and presses
	// its buttons.

	let page = $derived(downloads.page);

	async function open(id: string): Promise<void> {
		if (!inTauri()) {
			return;
		}
		const at = await downloadsOpen(id);
		if (at?.kind === 'game') {
			await goto(resolve('/game/[id]', { id: at.id }));
		} else if (at?.kind === 'store') {
			await goto(resolve('/store/[id]', { id: at.id }));
		}
	}

	onMount(() => {
		void downloads.refresh();
	});
</script>

<svelte:head>
	<title>Downloads · Zendo</title>
</svelte:head>

{#snippet jobButton(row: JobRow, kind: number)}
	{#if row.first}
		<button
			type="button"
			onclick={() => (kind === 0 ? downloads.pause(row.id) : downloads.resume(row.id))}
			class="border px-3 py-1 text-[13px] uppercase {row.first_primary
				? 'border-accent-mark text-accent-mark hover:bg-raised'
				: 'border-line text-foreground hover:bg-raised'}"
		>
			{row.first}
		</button>
	{/if}
	{#if row.cancel}
		<button
			type="button"
			onclick={() => downloads.cancel(row.id)}
			class="border border-line px-3 py-1 text-[13px] text-foreground uppercase hover:bg-raised"
		>
			{row.cancel}
		</button>
	{/if}
	{#if row.open}
		<button
			type="button"
			onclick={() => void open(row.id)}
			class="border border-line px-3 py-1 text-[13px] text-foreground uppercase hover:bg-raised"
		>
			{row.open}
		</button>
	{/if}
{/snippet}

{#snippet jobCard(row: JobRow, kind: number)}
	<li class="flex items-center gap-3 border border-line bg-surface px-4 py-2">
		<div class="min-w-0 flex-1">
			<p class="truncate text-[15px]">{row.title}</p>
			<p class="truncate text-xs text-dim">{row.line}</p>
			{#if row.note}
				<p class="truncate text-xs text-dim">{row.note}</p>
			{/if}
			{#if row.error}
				<p class="truncate text-xs text-bad">{row.error}</p>
			{/if}
		</div>
		{#if row.verdict}
			<p class="w-2/5 text-[13px] {row.verdict_bad ? 'text-bad' : 'text-good'}">{row.verdict}</p>
		{/if}
		{@render jobButton(row, kind)}
	</li>
{/snippet}

{#if !page}
	<p class="p-8 text-sm text-dim">{app.t('library-looking')}</p>
{:else}
	<div class="h-full overflow-y-auto p-6">
		<div class="mx-auto flex max-w-[900px] flex-col gap-4">
			{#if !page.empty}
				<div class="grid grid-cols-3 gap-3">
					<div class="border border-line bg-surface px-4 py-3">
						<p class="text-[11px] text-dim uppercase">{app.t('downloads-up-next')}</p>
						<p class="text-[22px] font-bold">{page.counts.up_next}</p>
					</div>
					<div class="border border-line bg-surface px-4 py-3">
						<p class="text-[11px] text-dim uppercase">{app.t('downloads-paused')}</p>
						<p class="text-[22px] font-bold">{page.counts.paused}</p>
					</div>
					<div class="border border-line bg-surface px-4 py-3">
						<p class="text-[11px] text-dim uppercase">{app.t('downloads-finished')}</p>
						<p class="text-[22px] font-bold">{page.counts.finished}</p>
					</div>
				</div>
			{/if}

			{#if page.running}
				{@const run = page.running}
				<div class="flex flex-col gap-3 border border-line bg-surface p-4">
					<div class="flex items-start gap-3">
						<div class="min-w-0 flex-1">
							<p class="truncate text-[18px] font-bold">{run.title}</p>
							<p class="truncate text-[13px] text-accent-mark">{run.line}</p>
						</div>
						<button
							type="button"
							disabled={!run.pause_on}
							onclick={() => downloads.pause(run.id)}
							class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-50"
						>
							{run.pause_label}
						</button>
						<button
							type="button"
							disabled={!run.cancel_on}
							onclick={() => downloads.cancel(run.id)}
							class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-50"
						>
							{run.cancel_label}
						</button>
					</div>
					{#if run.percent >= 0}
						<div class="h-2 bg-raised">
							<div class="h-full bg-accent-mark" style="width: {Math.min(run.percent, 100)}%"></div>
						</div>
					{/if}
					<p class="text-[13px] text-dim">{run.progress_line}</p>
				</div>
			{/if}

			{#if page.up_next.length > 0}
				<h2 class="text-xs font-bold text-dim uppercase">{app.t('downloads-up-next')}</h2>
				<ul class="flex flex-col gap-1">
					{#each page.up_next as row (row.id)}
						{@render jobCard(row, 0)}
					{/each}
				</ul>
			{/if}

			{#if page.paused.length > 0}
				<h2 class="text-xs font-bold text-dim uppercase">{app.t('downloads-paused')}</h2>
				<ul class="flex flex-col gap-1">
					{#each page.paused as row (row.id)}
						{@render jobCard(row, 1)}
					{/each}
				</ul>
			{/if}

			{#if page.finished.length > 0}
				<div class="flex items-center gap-3">
					<h2 class="flex-1 text-xs font-bold text-dim uppercase">
						{app.t('downloads-finished')}
					</h2>
					<button
						type="button"
						onclick={() => downloads.clear()}
						class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
					>
						{app.t('downloads-clear')}
					</button>
				</div>
				<ul class="flex flex-col gap-1">
					{#each page.finished as row (row.id)}
						{@render jobCard(row, 2)}
					{/each}
				</ul>
			{/if}

			{#if page.invite}
				<div class="flex flex-col items-center gap-3 pt-24 text-center">
					<p class="text-[18px]">{app.t('downloads-empty')}</p>
					<button
						type="button"
						onclick={() => void goto(resolve('/store'))}
						class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
					>
						{app.t('downloads-go-store')}
					</button>
				</div>
			{/if}

			{#if page.message}
				<p class="border-t border-line pt-2 text-sm text-dim">{page.message}</p>
			{/if}
		</div>
	</div>
{/if}
