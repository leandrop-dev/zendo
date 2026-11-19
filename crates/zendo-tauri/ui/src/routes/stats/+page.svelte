<script lang="ts">
	import { onMount } from 'svelte';
	import { inTauri, statsGet, statsPeriod, type StatsPage } from '#lib/api.js';
	import { app } from '#lib/state.svelte.js';

	// The Stats page: what the play sessions add up to, for a year or for all
	// of them. Rust counts; nothing is summed here.

	let page = $state<StatsPage | null>(null);
	let error = $state<string | null>(null);

	async function load(): Promise<void> {
		if (!inTauri()) {
			return;
		}
		try {
			page = await statsGet();
		} catch (err) {
			error = String(err);
		}
	}

	async function choose(at: number): Promise<void> {
		try {
			page = await statsPeriod(at);
		} catch (err) {
			error = String(err);
		}
	}

	// The arrows and Home/End move along the periods, as in the Slint window.
	function key(event: KeyboardEvent): void {
		if (!page || page.periods.length === 0) {
			return;
		}
		const last = page.periods.length - 1;
		let to: number;
		switch (event.key) {
			case 'ArrowLeft':
				to = Math.max(0, page.chosen - 1);
				break;
			case 'ArrowRight':
				to = Math.min(last, page.chosen + 1);
				break;
			case 'Home':
				to = 0;
				break;
			case 'End':
				to = last;
				break;
			default:
				return;
		}
		event.preventDefault();
		if (to !== page.chosen) {
			void choose(to);
		}
	}

	onMount(() => {
		void load();
		window.addEventListener('keydown', key);
		return () => window.removeEventListener('keydown', key);
	});
</script>

<svelte:head>
	<title>Stats · Zendo</title>
</svelte:head>

{#if error}
	<p class="p-8 text-sm text-bad">{error}</p>
{:else if !page}
	<p class="p-8 text-sm text-dim">{app.t('library-looking')}</p>
{:else if page.empty}
	<div class="flex h-full flex-col items-center justify-center gap-2 text-center">
		<p class="text-xl">{app.t('stats-none')}</p>
		<p class="max-w-[520px] text-sm text-dim">{app.t('stats-none-about')}</p>
	</div>
{:else}
	<div class="flex h-full min-h-0 flex-col">
		<div class="flex flex-wrap gap-2 px-6 pt-6 pb-2">
			{#each page.periods as chip, at (chip.label)}
				<button
					type="button"
					aria-pressed={chip.chosen}
					onclick={() => void choose(at)}
					class="border px-3 py-1 text-[13px] uppercase {chip.chosen
						? 'border-accent-mark text-accent-mark'
						: 'border-line text-foreground hover:bg-raised'}"
				>
					{chip.label}
				</button>
			{/each}
		</div>

		{#if page.none_period}
			<p class="px-6 py-8 text-sm text-dim">{app.t('stats-none-period')}</p>
		{:else}
			<div class="min-h-0 flex-1 overflow-y-auto p-6">
				<div class="mx-auto flex max-w-[1100px] flex-col gap-6">
					<div class="grid grid-cols-2 gap-3 md:grid-cols-4">
						{#each page.tiles as tile (tile.label)}
							<div class="border border-line bg-surface p-4">
								<p class="text-[11px] text-dim uppercase">{tile.label}</p>
								<p class="mt-1 text-xl">{tile.value}</p>
							</div>
						{/each}
					</div>

					<section class="border border-line bg-surface p-4">
						<h2 class="mb-3 text-[11px] text-dim uppercase">{app.t('stats-top')}</h2>
						<ul class="flex flex-col gap-2">
							{#each page.top as bar (bar.label)}
								<li class="flex items-center gap-4">
									<p class="w-1/2 truncate text-sm">{bar.label}</p>
									<div class="h-3 flex-1 bg-raised">
										<div class="h-full bg-accent-mark" style="width: {bar.share * 100}%"></div>
									</div>
									<p class="w-[90px] text-right text-sm text-dim">{bar.value}</p>
								</li>
							{/each}
						</ul>
					</section>

					<div class="grid grid-cols-1 gap-3 md:grid-cols-2">
						<section class="border border-line bg-surface p-4">
							<h2 class="mb-3 text-[11px] text-dim uppercase">{app.t('stats-by-month')}</h2>
							<div class="flex h-[160px] items-end gap-2">
								{#each page.months as col (col.label)}
									<div class="flex min-w-0 flex-1 flex-col items-center gap-1">
										<p class="text-[10px] text-dim">{col.value}</p>
										<div
											class="w-full bg-accent-mark"
											style="height: {Math.max(col.share * 120, col.share > 0 ? 2 : 0)}px"
										></div>
										<p class="truncate text-[10px] text-dim">{col.label}</p>
									</div>
								{/each}
							</div>
						</section>
						<section class="border border-line bg-surface p-4">
							<h2 class="mb-3 text-[11px] text-dim uppercase">{app.t('stats-by-weekday')}</h2>
							<div class="flex h-[160px] items-end gap-2">
								{#each page.weekdays as col (col.label)}
									<div class="flex min-w-0 flex-1 flex-col items-center gap-1">
										<p class="text-[10px] text-dim">{col.value}</p>
										<div
											class="w-full bg-accent-mark"
											style="height: {Math.max(col.share * 120, col.share > 0 ? 2 : 0)}px"
										></div>
										<p class="truncate text-[10px] text-dim">{col.label}</p>
									</div>
								{/each}
							</div>
						</section>
					</div>

					<section class="border border-line bg-surface p-4">
						<h2 class="mb-3 text-[11px] text-dim uppercase">{app.t('stats-facts')}</h2>
						<dl class="flex flex-col gap-1">
							{#each page.facts as fact (fact.label)}
								<div class="flex gap-4 text-[13px]">
									<dt class="w-[180px] shrink-0 text-dim">{fact.label}</dt>
									<dd class="min-w-0 flex-1 break-words">{fact.value}</dd>
								</div>
							{/each}
						</dl>
					</section>

					<section class="border border-line bg-surface p-4">
						<h2 class="mb-3 text-[11px] text-dim uppercase">{app.t('stats-by-status')}</h2>
						<div class="grid grid-cols-2 gap-3 md:grid-cols-4">
							{#each page.statuses as status (status.label)}
								<div class="border border-line p-3">
									<p class="text-[11px] text-dim uppercase">{status.label}</p>
									<p class="mt-1 text-lg">{status.value}</p>
								</div>
							{/each}
						</div>
					</section>
				</div>
			</div>
		{/if}
	</div>
{/if}
