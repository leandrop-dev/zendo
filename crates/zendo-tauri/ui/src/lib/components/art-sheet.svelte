<script lang="ts">
	import { convertFileSrc } from '@tauri-apps/api/core';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { onMount } from 'svelte';
	import { inTauri } from '#lib/api.js';
	import { artwork } from '#lib/artwork.svelte.js';
	import { app } from '#lib/state.svelte.js';

	// The art picker: the cover or wide image tabs, a search for the title,
	// the games found and the images to choose from. Rust keeps the picker
	// (`zendo_shell::artwork`); nothing here keeps a choice.

	let sheet = $derived(artwork.sheet);
	// The image under the pointer: its size, style and author.
	let hover = $state('');

	function asset(path: string): string {
		return inTauri() ? convertFileSrc(path) : '';
	}

	function search(): void {
		void artwork.search();
	}

	/// Settings on the Keys group, which is what the picker is asking for.
	function settings(): void {
		artwork.close();
		void goto(resolve('/settings?group=keys'));
	}

	// Escape closes; Enter in the field searches.
	function key(event: KeyboardEvent): void {
		if (!artwork.open) {
			return;
		}
		if (event.key === 'Escape') {
			event.preventDefault();
			artwork.close();
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
			onclick={() => artwork.close()}
		></button>
		<div
			class="relative flex max-h-[calc(100vh-2rem)] w-[min(980px,100%)] flex-col gap-4 overflow-hidden border border-line bg-background p-6"
		>
			<div class="flex shrink-0 items-center gap-3">
				<h2 class="min-w-0 flex-1 truncate text-lg font-bold uppercase">{sheet.heading}</h2>
				{#each sheet.tabs as tab (tab.id)}
					<button
						type="button"
						aria-pressed={tab.chosen}
						onclick={() => artwork.tab(tab.id)}
						class="border px-3 py-1 text-[13px] uppercase {tab.chosen
							? 'border-accent-mark text-accent-mark'
							: 'border-line text-foreground hover:bg-raised'}"
					>
						{tab.label}
					</button>
				{/each}
			</div>

			<div class="flex shrink-0 gap-3">
				<input
					type="text"
					value={artwork.query}
					placeholder={app.t('art-search-placeholder')}
					oninput={(event) => artwork.typed(event.currentTarget.value)}
					onkeydown={(event) => {
						if (event.key === 'Enter') {
							event.preventDefault();
							search();
						}
					}}
					class="min-w-0 flex-1 border border-line bg-surface px-3 py-1.5 text-sm"
				/>
				<button
					type="button"
					onclick={search}
					class="border border-line px-3 py-1.5 text-[13px] uppercase hover:bg-raised"
				>
					{app.t('art-search')}
				</button>
			</div>

			{#if sheet.games.length > 0}
				<div class="flex shrink-0 flex-wrap gap-2">
					{#each sheet.games as game (game.id)}
						<button
							type="button"
							aria-pressed={game.chosen}
							onclick={() => artwork.game(game.id)}
							class="border px-3 py-1 text-[13px] {game.chosen
								? 'border-accent-mark text-accent-mark'
								: 'border-line text-foreground hover:bg-raised'}"
						>
							{game.label}
						</button>
					{/each}
				</div>
			{/if}

			<div class="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto pr-2">
				<div class="flex flex-wrap gap-4">
					{#each sheet.thumbs as thumb (thumb.image)}
						<button
							type="button"
							disabled={sheet.busy}
							onclick={() => artwork.choose(thumb.image)}
							onmouseenter={() => (hover = thumb.about)}
							onmouseleave={() => (hover = '')}
							onfocus={() => (hover = thumb.about)}
							onblur={() => (hover = '')}
							class="relative shrink-0 overflow-hidden border-2 border-transparent bg-surface hover:border-accent-mark {sheet.kind ===
							0
								? 'h-[180px] w-[120px]'
								: 'h-[93px] w-[288px]'}"
						>
							{#if thumb.path}
								<img
									src={asset(thumb.path)}
									alt=""
									loading="lazy"
									class="h-full w-full object-cover"
								/>
							{/if}
						</button>
					{/each}
				</div>
				{#if sheet.more}
					<button
						type="button"
						disabled={sheet.busy}
						onclick={() => artwork.more()}
						class="self-start border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-50"
					>
						{app.t('art-show-more')}
					</button>
				{/if}
			</div>

			<p class="h-4 shrink-0 truncate text-[12px] text-dim">{hover}</p>

			<div class="flex shrink-0 flex-wrap items-center gap-3 border-t border-line pt-4">
				{#if sheet.busy}
					<p class="min-w-0 flex-1 text-[13px] text-dim">{sheet.status}</p>
				{:else}
					<p class="min-w-0 flex-1 text-[13px] text-bad">{sheet.note}</p>
				{/if}
				{#if sheet.needs_key}
					<button
						type="button"
						onclick={settings}
						class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
					>
						{app.t('art-open-settings')}
					</button>
				{/if}
				{#if sheet.automatic_label}
					<button
						type="button"
						onclick={() => artwork.automatic()}
						class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
					>
						{sheet.automatic_label}
					</button>
				{/if}
				<button
					type="button"
					onclick={() => artwork.close()}
					class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
				>
					{app.t('close')}
				</button>
			</div>
		</div>
	</div>
{/if}
