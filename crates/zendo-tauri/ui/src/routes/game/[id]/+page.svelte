<script lang="ts">
	import { convertFileSrc } from '@tauri-apps/api/core';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { listen } from '@tauri-apps/api/event';
	import { onMount } from 'svelte';
	import {
		gameGet,
		gameLaunch,
		gameSetHidden,
		gameSetPinned,
		gameSetStatus,
		gameStop,
		inTauri,
		type GamePage
	} from '#lib/api.js';
	import { app } from '#lib/state.svelte.js';

	// A game's page: the wide art faint behind the header, the cover, the
	// title, the actions, the status chips and the four tiles. The cards
	// under them are the next parts of this slice.
	let game = $state<GamePage | null>(null);
	let error = $state<string | null>(null);
	let id = $derived(page.params.id ?? '');

	let cover = $derived(
		game?.cover !== null && game?.cover && inTauri() ? convertFileSrc(game.cover) : null
	);
	let hero = $derived(
		game?.hero !== null && game?.hero && inTauri() ? convertFileSrc(game.hero) : null
	);

	async function load(): Promise<void> {
		if (!inTauri() || !id) {
			return;
		}
		try {
			game = await gameGet(id);
		} catch (err) {
			error = String(err);
		}
	}

	async function act(action: () => Promise<GamePage>): Promise<void> {
		try {
			game = await action();
		} catch (err) {
			error = String(err);
		}
	}

	function launch(): void {
		if (game && game.run_state === 0) {
			void act(() => gameLaunch(id));
		}
	}

	function stop(): void {
		if (game) {
			void act(() => gameStop(id));
		}
	}

	function back(): void {
		void goto(resolve('/'));
	}

	function key(event: KeyboardEvent): void {
		if (event.key === 'Escape') {
			event.preventDefault();
			back();
		}
	}

	onMount(() => {
		void load();
		window.addEventListener('keydown', key);
		const unlisten = listen<{ id: string; state: number }>('game_run', (event) => {
			if (game && event.payload.id === game.id) {
				game.run_state = event.payload.state;
			}
		});
		const lines = listen<{ id: string; line: string }>('game_line', (event) => {
			if (game && event.payload.id === game.id) {
				game.line = event.payload.line;
			}
		});
		return () => {
			window.removeEventListener('keydown', key);
			void unlisten.then((stop) => stop());
			void lines.then((stop) => stop());
		};
	});
</script>

<svelte:head>
	<title>{game ? `${game.title} · Zendo` : 'Zendo'}</title>
</svelte:head>

{#if error}
	<p class="p-8 text-sm text-bad">{error}</p>
{:else if !game}
	<p class="p-8 text-sm text-dim">{app.t('library-looking')}</p>
{:else}
	<div class="relative h-full overflow-y-auto">
		{#if hero}
			<img
				src={hero}
				alt=""
				class="pointer-events-none absolute inset-x-0 top-0 h-[340px] w-full object-cover opacity-20"
			/>
		{/if}

		<div class="relative flex flex-col gap-6 p-6">
			<button
				type="button"
				onclick={back}
				class="self-start text-[13px] text-dim uppercase hover:text-foreground"
			>
				{app.t('game-back')}
			</button>

			<div class="flex gap-6">
				<div class="relative h-[270px] w-[180px] shrink-0 overflow-hidden rounded-[2px] bg-surface">
					{#if cover}
						<img src={cover} alt="" class="h-full w-full object-cover" />
					{:else}
						<span class="flex h-full w-full items-center justify-center p-3 text-center text-lg">
							{game.title}
						</span>
					{/if}
				</div>

				<div class="flex min-w-0 flex-col gap-3">
					<div class="flex flex-wrap items-center gap-2">
						<h1 class="text-3xl font-bold tracking-[0.1em] uppercase">{game.title}</h1>
						{#if game.pinned}
							<span
								class="border border-accent-mark px-1.5 py-0.5 text-[11px] font-bold text-accent-mark"
							>
								{app.t('badge-favorite')}
							</span>
						{/if}
						{#if game.hidden}
							<span class="border border-line px-1.5 py-0.5 text-[11px] font-bold text-dim">
								{app.t('badge-hidden')}
							</span>
						{/if}
					</div>
					<p class="text-sm text-dim">{game.source_line}</p>

					<div class="mt-2 flex flex-wrap gap-2">
						<button
							type="button"
							disabled={game.run_state !== 0}
							onclick={launch}
							class="min-w-[130px] border border-accent-mark bg-accent-mark px-4 py-2 text-[13px] font-bold text-background uppercase disabled:opacity-60"
						>
							{game.run_state === 0
								? app.t('run-play')
								: game.run_state === 1
									? app.t('run-state-starting')
									: app.t('run-state-running')}
						</button>
						{#if game.run_state >= 2}
							<button
								type="button"
								onclick={stop}
								class="border border-bad px-4 py-2 text-[13px] text-bad uppercase"
							>
								{game.run_state === 3 ? app.t('run-force-stop') : app.t('run-stop')}
							</button>
						{/if}
						<button
							type="button"
							aria-pressed={game.pinned}
							onclick={() => void act(() => gameSetPinned(id, !game!.pinned))}
							class="border px-4 py-2 text-[13px] uppercase {game.pinned
								? 'border-accent-mark text-accent-mark'
								: 'border-line text-foreground hover:bg-raised'}"
						>
							{game.pinned ? app.t('game-unfavorite') : app.t('game-favorite')}
						</button>
						<button
							type="button"
							aria-pressed={game.hidden}
							onclick={() => void act(() => gameSetHidden(id, !game!.hidden))}
							class="border px-4 py-2 text-[13px] uppercase {game.hidden
								? 'border-accent-mark text-accent-mark'
								: 'border-line text-foreground hover:bg-raised'}"
						>
							{game.hidden ? app.t('game-unhide') : app.t('game-hide')}
						</button>
					</div>

					<p class="text-sm text-dim">{game.line}</p>
				</div>
			</div>

			<div class="flex flex-wrap gap-2">
				{#each game.statuses as status (status.key)}
					<button
						type="button"
						aria-pressed={status.chosen}
						onclick={() => void act(() => gameSetStatus(id, status.chosen ? null : status.key))}
						class="border px-3 py-1 text-[13px] uppercase {status.chosen
							? 'border-accent-mark text-accent-mark'
							: 'border-line text-foreground hover:bg-raised'}"
					>
						{status.label}
					</button>
				{/each}
			</div>

			<div class="grid grid-cols-2 gap-3 md:grid-cols-4">
				{#each game.tiles as tile (tile.label)}
					<div class="border border-line bg-surface p-4">
						<p class="text-[11px] text-dim uppercase">{tile.label}</p>
						<p class="mt-1 text-xl">{tile.value}</p>
					</div>
				{/each}
			</div>
		</div>
	</div>
{/if}
