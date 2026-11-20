<script lang="ts">
	import { convertFileSrc } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import { inTauri } from '#lib/api.js';
	import { gameMode } from '#lib/game_mode.svelte.js';
	import { app } from '#lib/state.svelte.js';

	// Game Mode's screen: one game large, a row of covers under it, and the
	// Start menu. Rust keeps the shelf, the hero and the menu.

	let page = $derived(gameMode.page);
	let hero = $derived(page?.hero ?? null);
	let cover = $derived(hero?.cover ? convertFileSrc(hero.cover) : null);

	function art(path: string | null): string {
		return path !== null && inTauri() ? convertFileSrc(path) : '';
	}

	onMount(() => {
		const resize = () => gameMode.sized(window.innerWidth);
		window.addEventListener('resize', resize);
		resize();
		return () => window.removeEventListener('resize', resize);
	});
</script>

{#if gameMode.screen && page}
	<div class="fixed inset-0 z-50 flex flex-col bg-background">
		<div class="flex items-center justify-end gap-2 px-8 pt-6">
			{#each page.shelves as shelf, at (shelf.label)}
				<button
					type="button"
					aria-pressed={shelf.chosen}
					onclick={() => gameMode.shelf(at)}
					class="border px-3 py-1 text-[13px] uppercase {shelf.chosen
						? 'border-accent-mark text-accent-mark'
						: 'border-line text-foreground hover:bg-raised'}"
				>
					{shelf.label}
					<span class="ml-1.5 text-xs {shelf.chosen ? 'text-accent-mark' : 'text-dim'}">
						{shelf.count}
					</span>
				</button>
			{/each}
		</div>

		{#if page.empty}
			<div class="flex flex-1 items-center justify-center">
				<p class="text-lg text-dim">{app.t('gm-empty')}</p>
			</div>
		{:else if hero}
			<div class="flex min-h-0 flex-1 items-center gap-10 px-12 py-6">
				<div class="h-[420px] w-[280px] shrink-0 overflow-hidden rounded-[2px] bg-surface">
					{#if cover}
						<img src={cover} alt="" class="h-full w-full object-cover" />
					{:else}
						<span class="flex h-full w-full items-center justify-center p-4 text-center text-xl">
							{hero.title}
						</span>
					{/if}
				</div>
				<div class="flex min-w-0 flex-col gap-3">
					<p class="text-xs tracking-[0.2em] text-accent-mark uppercase">{hero.meta}</p>
					<h1 class="text-4xl font-bold tracking-[0.08em] uppercase">{hero.title}</h1>
					<p class="max-w-[640px] text-sm whitespace-pre-line text-dim">{hero.description}</p>
					<div class="mt-3 flex gap-3">
						<button
							type="button"
							onclick={() => gameMode.activate()}
							class="min-w-[140px] border border-accent-mark bg-accent-mark px-5 py-2 text-[13px] font-bold text-background uppercase"
						>
							{app.t('run-play')}
						</button>
						<button
							type="button"
							onclick={() => gameMode.details()}
							class="border border-line px-5 py-2 text-[13px] text-foreground uppercase hover:bg-raised"
						>
							{app.t('game-mode-details')}
						</button>
						<button
							type="button"
							onclick={() => gameMode.openMenu()}
							class="border border-line px-5 py-2 text-[13px] text-foreground uppercase hover:bg-raised"
						>
							{app.t('gm-menu')}
						</button>
					</div>
				</div>
			</div>
		{/if}

		<div class="flex justify-center gap-2 px-8 pb-8">
			{#each page.tiles as tile (tile.title)}
				{@const current = tile.current}
				<button
					type="button"
					onclick={() => gameMode.pick(page.tiles.indexOf(tile))}
					class="h-[150px] w-[110px] shrink-0 overflow-hidden border-2 bg-surface {current
						? 'border-accent-mark'
						: 'border-transparent hover:border-line'}"
				>
					{#if tile.cover}
						<img src={art(tile.cover)} alt="" class="h-full w-full object-cover" />
					{:else}
						<span
							class="flex h-full w-full items-center justify-center p-1 text-center text-[11px]"
						>
							{tile.title}
						</span>
					{/if}
				</button>
			{/each}
		</div>

		{#if page.menu_open}
			<div class="absolute inset-0 grid place-items-center bg-background/80">
				<div class="flex w-[360px] flex-col gap-2 border border-line bg-surface p-6">
					<p class="mb-2 text-xs text-dim uppercase">{app.t('gm-menu')}</p>
					{#each page.menu as choice, at (choice.label)}
						<button
							type="button"
							aria-pressed={choice.chosen}
							onclick={() => gameMode.choose(at)}
							class="border px-3 py-2 text-left text-[14px] uppercase {choice.chosen
								? 'border-accent-mark text-accent-mark'
								: 'border-line text-foreground hover:bg-raised'}"
						>
							{choice.label}
						</button>
					{/each}
				</div>
			</div>
		{/if}
	</div>
{/if}
