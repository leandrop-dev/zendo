<script lang="ts">
	import { convertFileSrc } from '@tauri-apps/api/core';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { listen } from '@tauri-apps/api/event';
	import { onDestroy, onMount } from 'svelte';
	import {
		cardsGet,
		cardsMoreAchievements,
		cardsMoreChangelog,
		cardsRetry,
		gameCheckUpdates,
		gameDlc,
		gameDlcAll,
		gameGet,
		gameLaunch,
		gameMove,
		gameOpen,
		gameRepair,
		gameToggleMore,
		gameUninstall,
		gameUpdate,
		gameSetHidden,
		gameSetPinned,
		gameSetStatus,
		gameStop,
		inTauri,
		localRemove,
		modsAdd,
		modsApply,
		modsBegin,
		modsClearOverwrite,
		modsFindNexus,
		modsFolderKnown,
		modsFolderSave,
		modsGet,
		modsMoveMod,
		modsMovePlugin,
		modsProfileCancel,
		modsProfileDelete,
		modsProfileNaming,
		modsProfileSave,
		modsProfileSwitch,
		modsRemove,
		modsReinstall,
		modsToggleMod,
		modsTogglePlugin,
		nexusGet,
		nexusStop,
		nexusUpdate,
		openLink,
		prefixInstall,
		prefixOpen,
		prefixPickProgram,
		prefixRun,
		prefixWant,
		steamAppSet,
		steamUninstallGame,
		steamVerifyGame,
		type GameCards,
		type GamePage,
		type ModsCard,
		type NexusDownload,
		type Pill
	} from '#lib/api.js';
	import ArtSheet from '#lib/components/art-sheet.svelte';
	import LocalForm from '#lib/components/local-form.svelte';
	import OptionsSheet from '#lib/components/options-sheet.svelte';
	import SearchSheet from '#lib/components/search-sheet.svelte';
	import { artwork } from '#lib/artwork.svelte.js';
	import { installer } from '#lib/installer.svelte.js';
	import { options } from '#lib/options.svelte.js';
	import { search } from '#lib/search.svelte.js';
	import { app } from '#lib/state.svelte.js';

	// A game's page: the wide art faint behind the header, the cover, the
	// title, the actions, the status chips and the four tiles, then the cards
	// the stores and the sites fill: Steam's about, screenshots and facts,
	// the achievements, the requirements, the Wine prefix, the DLC and the
	// game's details.
	let game = $state<GamePage | null>(null);
	let cards = $state<GameCards | null>(null);
	let error = $state<string | null>(null);
	// A game added by hand: the sheet to edit it, and Remove pressed once.
	let editing = $state(false);
	let removing = $state(false);
	// The Steam app id's field and what saving it said.
	let appInput = $state('');
	let appNote = $state<string | null>(null);
	// The winetricks components typed, and a tool that could not start.
	let prefixInput = $state('');
	let prefixNote = $state<string | null>(null);
	// The screenshot lightbox: the one open, if any.
	let shot = $state<number | null>(null);
	// The mods coming in from Nexus for this game.
	let downloads = $state<NexusDownload[]>([]);
	// The Mods card, the folder and profile name being typed, and a change
	// that could not start.
	let mods = $state<ModsCard | null>(null);
	let modsNote = $state<string | null>(null);
	let folderInput = $state('');
	let profileName = $state('');
	let id = $derived(page.params.id ?? '');

	let cover = $derived(
		game?.cover !== null && game?.cover && inTauri() ? convertFileSrc(game.cover) : null
	);
	let hero = $derived(
		game?.hero !== null && game?.hero && inTauri() ? convertFileSrc(game.hero) : null
	);
	// The details card: what the game is, then what only Steam says.
	let detailRows = $derived([
		...(game?.details ?? []).map((row) => ({ ...row, url: null as string | null })),
		...(cards?.steam ?? []).map((row) => ({ ...row, tone: 0 }))
	]);
	// Every file of the cards is in Zendo's cache; the asset protocol serves it.
	function asset(path: string | null): string {
		return path !== null && inTauri() ? convertFileSrc(path) : '';
	}

	async function load(opening = false): Promise<void> {
		if (!inTauri() || !id) {
			return;
		}
		try {
			game = opening ? await gameOpen(id) : await gameGet(id);
			// The components in the box are read once; what is typed after
			// that is the page's own.
			if (prefixInput === '' && game.prefix?.components) {
				prefixInput = game.prefix.components;
			}
		} catch (err) {
			error = String(err);
		}
	}

	async function loadCards(): Promise<void> {
		if (!inTauri() || !id) {
			return;
		}
		try {
			const first = cards === null;
			cards = await cardsGet(id);
			// An answer from another card must not clear what is being typed.
			if (first) {
				appInput = cards.steam_app.input;
			}
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

	async function moreAchievements(): Promise<void> {
		try {
			cards = await cardsMoreAchievements(id);
		} catch (err) {
			error = String(err);
		}
	}

	async function moreChangelog(): Promise<void> {
		try {
			cards = await cardsMoreChangelog(id);
		} catch (err) {
			error = String(err);
		}
	}

	async function retry(): Promise<void> {
		try {
			cards = await cardsRetry(id);
		} catch (err) {
			error = String(err);
		}
	}

	async function saveApp(appId: string | null): Promise<void> {
		try {
			appNote = await steamAppSet(id, appId);
			cards = await cardsGet(id);
			appInput = cards.steam_app.input;
		} catch (err) {
			appNote = String(err);
		}
	}

	// A prefix tool: it holds the card while it runs, and says why it could
	// not start.
	async function runTool(tool: string): Promise<void> {
		prefixNote = null;
		try {
			await prefixRun(id, tool);
		} catch (err) {
			prefixNote = String(err);
		}
	}

	async function installComponents(): Promise<void> {
		prefixNote = null;
		try {
			await prefixInstall(id, prefixInput);
			prefixInput = '';
		} catch (err) {
			prefixNote = String(err);
		}
	}

	async function pickProgram(): Promise<void> {
		prefixNote = null;
		try {
			await prefixPickProgram(id);
		} catch (err) {
			prefixNote = String(err);
		}
	}

	async function loadMods(open: boolean): Promise<void> {
		if (!inTauri() || !id) {
			return;
		}
		try {
			mods = open ? await modsBegin(id) : await modsGet(id);
			downloads = (await nexusGet(id)).downloads;
			if (mods.folder) {
				folderInput = mods.folder;
			}
			if (mods.naming) {
				profileName = mods.naming.name;
			}
		} catch (err) {
			modsNote = String(err);
		}
	}

	/// A Nexus press: the card and the downloads follow through
	/// `nexus_changed`.
	async function nexusAct(action: () => Promise<unknown>): Promise<void> {
		try {
			await action();
		} catch (err) {
			modsNote = String(err);
		}
	}

	async function stopDownload(at: number): Promise<void> {
		try {
			downloads = (await nexusStop(at, id)).downloads;
		} catch (err) {
			modsNote = String(err);
		}
	}

	async function modsAct(action: () => Promise<ModsCard>): Promise<void> {
		modsNote = null;
		try {
			mods = await action();
			if (mods.folder) {
				folderInput = mods.folder;
			}
			if (mods.naming) {
				profileName = mods.naming.name;
			}
		} catch (err) {
			modsNote = String(err);
		}
	}

	async function addMod(): Promise<void> {
		modsNote = null;
		try {
			await modsAdd(id);
		} catch (err) {
			modsNote = String(err);
		}
	}

	async function findOnNexus(): Promise<void> {
		try {
			await modsFindNexus(id);
		} catch (err) {
			modsNote = String(err);
		}
	}

	function stepShot(by: number): void {
		if (shot !== null && cards && cards.shots.length > 0) {
			const count = cards.shots.length;
			shot = (((shot + by) % count) + count) % count;
		}
	}

	function key(event: KeyboardEvent): void {
		// The sheets keep their own keys while they are open.
		if (options.open || artwork.open || search.open || installer.open) {
			return;
		}
		if (shot !== null) {
			if (event.key === 'Escape') {
				event.preventDefault();
				shot = null;
			} else if (event.key === 'ArrowLeft') {
				event.preventDefault();
				stepShot(-1);
			} else if (event.key === 'ArrowRight') {
				event.preventDefault();
				stepShot(1);
			}
			return;
		}
		if (event.key === 'Escape' && !editing) {
			event.preventDefault();
			back();
		}
	}

	// The pill's colour: its own tier colour, else the theme's for its tone.
	function pillStyle(pill: Pill): string {
		return pill.tone === 0 && pill.tint ? `border-color: ${pill.tint}; color: ${pill.tint}` : '';
	}

	function pillClass(pill: Pill): string {
		switch (pill.tone) {
			case 0:
				return 'border';
			case 1:
				return 'border border-good text-good';
			case 2:
				return 'border border-bad text-bad';
			default:
				return 'border border-line text-dim';
		}
	}

	// The anti-cheat status's colour: 1 good, 2 bad, 3 dim.
	function statusClass(tone: number): string {
		switch (tone) {
			case 1:
				return 'border border-good text-good';
			case 2:
				return 'border border-bad text-bad';
			case 3:
				return 'border border-line text-dim';
			default:
				return 'border border-line';
		}
	}

	// Leaving the page puts the sheets away.
	onDestroy(() => {
		options.dismiss();
		artwork.dismiss();
		search.dismiss();
	});

	onMount(() => {
		void load(true);
		void loadCards();
		void loadMods(true);
		void prefixWant(id);
		window.addEventListener('keydown', key);
		const runs = listen<{ id: string; state: number }>('game_run', (event) => {
			if (game && event.payload.id === game.id) {
				game.run_state = event.payload.state;
				// The end of a game writes a session and its minutes: ask again.
				if (event.payload.state === 0) {
					void load();
				}
			}
		});
		const lines = listen<{ id: string; line: string }>('game_line', (event) => {
			if (game && event.payload.id === game.id) {
				game.line = event.payload.line;
			}
		});
		// An answer to a card came in: ask for them all again.
		const arrived = listen<string>('game_cards', (event) => {
			if (event.payload === id) {
				void loadCards();
			}
		});
		// A prefix tool ended, the known fixes were read again, or the
		// game's saves' state changed.
		const changed = listen<string>('game_changed', (event) => {
			if (event.payload === id) {
				void load();
				void loadMods(false);
			}
		});
		// The library was scanned again (the automatic image back): the
		// page's own copy follows.
		const loaded = listen('library_loaded', () => void load());
		// A job queued, paused or done, or Steam's own work: the line and the
		// buttons follow.
		const queued = listen('downloads_changed', () => void load());
		const steamed = listen('steam_changed', () => void load());
		// A mod from Nexus started, moved on or ended, or Update was pressed.
		const nexusMods = listen('nexus_changed', () => void loadMods(false));
		return () => {
			window.removeEventListener('keydown', key);
			void runs.then((stop) => stop());
			void lines.then((stop) => stop());
			void arrived.then((stop) => stop());
			void changed.then((stop) => stop());
			void loaded.then((stop) => stop());
			void queued.then((stop) => stop());
			void steamed.then((stop) => stop());
			void nexusMods.then((stop) => stop());
		};
	});
</script>

<svelte:head>
	<title>{game ? `${game.title} · Zendo` : 'Zendo'}</title>
</svelte:head>

{#snippet antiCheat(card: NonNullable<GameCards['anti_cheat']>)}
	<section class="border border-line bg-surface p-4">
		<h2 class="mb-3 text-[11px] text-dim uppercase">{app.t('anti-cheat-title')}</h2>
		<div class="flex flex-wrap items-center gap-2">
			<span class="px-1.5 py-0.5 text-[11px] font-bold {statusClass(card.tone)}">
				{card.label}
			</span>
			{#if card.makers}
				<p class="text-sm text-dim">{card.makers}</p>
			{/if}
		</div>
		<p class="mt-2 text-sm">{card.about}</p>
		{#each card.notes as note (note.text)}
			{#if note.url}
				<button
					type="button"
					onclick={() => void openLink(note.url)}
					class="mt-1 block text-left text-sm text-accent-mark underline"
				>
					{note.text}
				</button>
			{:else}
				<p class="mt-1 text-sm text-dim">{note.text}</p>
			{/if}
		{/each}
		<button
			type="button"
			onclick={() => void openLink(card.url)}
			class="mt-3 border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
		>
			{app.t('anti-cheat-open')}
		</button>
	</section>
{/snippet}

{#if shot !== null && cards && cards.shots[shot]}
	<div class="fixed inset-0 z-50 flex flex-col gap-4 bg-background/95 p-6">
		<button
			type="button"
			onclick={() => (shot = null)}
			class="min-h-0 flex-1"
			aria-label={app.t('close')}
		>
			<img src={asset(cards.shots[shot].full)} alt="" class="h-full w-full object-contain" />
		</button>
		<div class="flex items-center justify-center gap-3">
			<button
				type="button"
				onclick={() => stepShot(-1)}
				class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
			>
				{app.t('shots-previous')}
			</button>
			<p class="text-sm text-dim">{shot + 1} / {cards.shots.length}</p>
			<button
				type="button"
				onclick={() => stepShot(1)}
				class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
			>
				{app.t('shots-next')}
			</button>
			<button
				type="button"
				onclick={() => (shot = null)}
				class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
			>
				{app.t('close')}
			</button>
		</div>
	</div>
{/if}

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
				<div class="flex shrink-0 flex-col gap-2">
					<div class="relative h-[270px] w-[180px] overflow-hidden rounded-[2px] bg-surface">
						{#if cover}
							<img src={cover} alt="" class="h-full w-full object-cover" />
						{:else}
							<span class="flex h-full w-full items-center justify-center p-3 text-center text-lg">
								{game.title}
							</span>
						{/if}
					</div>
					<button
						type="button"
						onclick={() => void artwork.openFor(id, 0)}
						class="border border-line px-2 py-1 text-[12px] uppercase hover:bg-raised"
					>
						{app.t('game-change-cover')}
					</button>
					<button
						type="button"
						onclick={() => void artwork.openFor(id, 1)}
						class="border border-line px-2 py-1 text-[12px] uppercase hover:bg-raised"
					>
						{app.t('game-change-backdrop')}
					</button>
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
					<div class="flex flex-wrap items-center gap-2">
						<p class="text-sm text-dim">{game.source_line}</p>
						{#if cards?.pill}
							{#if cards.pill.url}
								<button
									type="button"
									title={cards.pill.tip}
									style={pillStyle(cards.pill)}
									onclick={() => void openLink(cards!.pill!.url)}
									class="px-1.5 py-0.5 text-[11px] font-bold {pillClass(cards.pill)}"
								>
									{cards.pill.label}
								</button>
							{:else}
								<span
									title={cards.pill.tip}
									style={pillStyle(cards.pill)}
									class="px-1.5 py-0.5 text-[11px] font-bold {pillClass(cards.pill)}"
								>
									{cards.pill.label}
								</span>
							{/if}
						{/if}
						{#if cards?.deck}
							<button
								type="button"
								title={cards.deck.tip}
								style={pillStyle(cards.deck)}
								onclick={() => void openLink(cards!.deck!.url)}
								class="px-1.5 py-0.5 text-[11px] font-bold {pillClass(cards.deck)}"
							>
								{cards.deck.label}
							</button>
						{/if}
					</div>

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
						{#if game.update.label}
							<button
								type="button"
								disabled={!game.update.on}
								onclick={() => void act(() => gameUpdate(id))}
								class="border border-accent-mark px-4 py-2 text-[13px] text-accent-mark uppercase hover:bg-raised disabled:opacity-50"
							>
								{game.update.label}
							</button>
						{/if}
						{#if game.launch_options}
							<button
								type="button"
								onclick={() => void options.openFor(id)}
								class="border border-line px-4 py-2 text-[13px] text-foreground uppercase hover:bg-raised"
							>
								{app.t('game-launch-options')}
							</button>
						{/if}
						{#if game.local}
							<button
								type="button"
								onclick={() => (editing = true)}
								class="border border-line px-4 py-2 text-[13px] text-foreground uppercase hover:bg-raised"
							>
								{app.t('edit')}
							</button>
							<button
								type="button"
								onclick={async () => {
									if (!removing) {
										removing = true;
									} else {
										await localRemove(id);
										back();
									}
								}}
								class="border border-bad px-4 py-2 text-[13px] text-bad uppercase"
							>
								{removing ? app.t('really-remove') : app.t('remove')}
							</button>
						{/if}
						{#if game.manage.has_more}
							<button
								type="button"
								aria-expanded={game.manage.more_open}
								onclick={() => void act(() => gameToggleMore(id))}
								class="border border-line px-4 py-2 text-[13px] text-foreground uppercase hover:bg-raised"
							>
								{game.manage.more_open ? app.t('game-less') : app.t('game-more')}
							</button>
						{/if}
					</div>

					{#if game.manage.has_more && game.manage.more_open}
						<div class="flex flex-wrap gap-2 border border-line bg-surface px-3 py-2">
							{#if game.manage.store_managed}
								<button
									type="button"
									disabled={!game.manage.check_on}
									onclick={() => void act(() => gameCheckUpdates(id))}
									class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-50"
								>
									{app.t('downloads-check-updates')}
								</button>
								<button
									type="button"
									disabled={!game.manage.job_free}
									onclick={() => void act(() => gameRepair(id))}
									class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-50"
								>
									{game.manage.repair_label}
								</button>
								<button
									type="button"
									disabled={!game.manage.move_on}
									onclick={() => void act(() => gameMove(id))}
									class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-50"
								>
									{app.t('downloads-move')}
								</button>
							{/if}
							{#if game.manage.managed}
								<button
									type="button"
									disabled={!game.manage.job_free}
									onclick={() => void act(() => gameUninstall(id))}
									class="border px-3 py-1 text-[13px] uppercase disabled:opacity-50 {game.manage
										.uninstall_again
										? 'border-bad text-bad'
										: 'border-line text-foreground hover:bg-raised'}"
								>
									{game.manage.uninstall_again
										? app.t('downloads-press-again')
										: game.manage.queue_uninstall_label}
								</button>
							{/if}
							{#if game.manage.verify_label}
								<button
									type="button"
									disabled={!game.manage.verify_on}
									onclick={() => void act(() => steamVerifyGame(id))}
									class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-50"
								>
									{game.manage.verify_label}
								</button>
							{/if}
							{#if game.manage.uninstall_label}
								<button
									type="button"
									onclick={() => void act(() => steamUninstallGame(id))}
									class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
								>
									{game.manage.uninstall_label}
								</button>
							{/if}
						</div>
					{/if}

					<div class="flex items-center gap-3">
						<p class="text-sm text-dim">{game.job.shown ? game.job.line : game.line}</p>
						{#if game.job.shown}
							<button
								type="button"
								onclick={() => void goto(resolve('/downloads'))}
								class="border border-line px-2 py-0.5 text-[12px] uppercase hover:bg-raised"
							>
								{app.t('see-downloads')}
							</button>
						{/if}
					</div>
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

			{#if cards?.anti_cheat?.warns}
				{@render antiCheat(cards.anti_cheat)}
			{/if}

			{#if cards?.about}
				<section class="border border-line bg-surface p-4">
					<h2 class="mb-3 text-[11px] text-dim uppercase">{app.t('game-about')}</h2>
					<p class="text-sm whitespace-pre-line">{cards.about}</p>
				</section>
			{:else if cards?.no_match}
				<section class="border border-line bg-surface p-4">
					<h2 class="mb-3 text-[11px] text-dim uppercase">{app.t('game-about')}</h2>
					<p class="text-sm text-dim">{cards.no_match}</p>
				</section>
			{/if}

			{#if cards?.anti_cheat && !cards.anti_cheat.warns}
				{@render antiCheat(cards.anti_cheat)}
			{/if}

			{#if cards && cards.shots.length > 0}
				<section class="border border-line bg-surface p-4">
					<h2 class="mb-3 text-[11px] text-dim uppercase">{app.t('screenshots-title')}</h2>
					<div class="flex gap-3 overflow-x-auto">
						{#each cards.shots as picture, at (picture.thumb)}
							<button
								type="button"
								onclick={() => (shot = at)}
								class="shrink-0"
								aria-label={app.t('screenshots-title')}
							>
								<img
									src={asset(picture.thumb)}
									alt=""
									loading="lazy"
									class="h-[150px] rounded-[2px] object-cover"
								/>
							</button>
						{/each}
					</div>
				</section>
			{/if}

			{#if cards && cards.achievements.rows.length > 0}
				<section class="border border-line bg-surface p-4">
					<h2 class="mb-3 text-[11px] text-dim uppercase">{app.t('achievements-title')}</h2>
					<div class="flex items-center gap-3">
						<p class="text-sm {cards.achievements.share === null ? 'text-dim' : ''}">
							{cards.achievements.summary}
						</p>
						{#if cards.achievements.share !== null}
							<div class="h-1.5 flex-1 bg-raised">
								<div
									class="h-full bg-accent-mark"
									style="width: {Math.round(
										Math.min(1, Math.max(0, cards.achievements.share)) * 100
									)}%"
								></div>
							</div>
						{/if}
					</div>
					<ul class="mt-4 flex flex-col gap-3">
						{#each cards.achievements.rows as row (row.name + row.side)}
							<li class="flex items-center gap-3">
								<div class="flex h-11 w-11 shrink-0 items-center justify-center bg-raised">
									{#if row.icon}
										<img src={asset(row.icon)} alt="" class="h-full w-full object-cover" />
									{:else}
										<span class="text-dim">?</span>
									{/if}
								</div>
								<div class="min-w-0 flex-1">
									<p class="text-sm font-bold">{row.name}</p>
									{#if row.description}
										<p class="text-[13px] text-dim">{row.description}</p>
									{/if}
								</div>
								<span class="shrink-0 text-[13px] {row.unlocked ? 'text-good' : 'text-dim'}">
									{row.side}
								</span>
							</li>
						{/each}
					</ul>
					{#if cards.achievements.more}
						<button
							type="button"
							onclick={() => void moreAchievements()}
							class="mt-4 border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
						>
							{cards.achievements.more}
						</button>
					{/if}
				</section>
			{/if}

			{#if cards && cards.requirements.rows.length > 0}
				<section class="border border-line bg-surface p-4">
					<h2 class="mb-3 text-[11px] text-dim uppercase">{app.t('requirements-title')}</h2>
					<p class="mb-3 text-[13px] text-dim">{cards.requirements.source}</p>
					<div class="flex flex-col gap-2">
						{#if cards.requirements.both}
							<div class="flex gap-3">
								<span class="w-[130px] shrink-0"></span>
								<span class="flex-1 text-sm font-bold text-dim">
									{app.t('requirements-minimum')}
								</span>
								<span class="flex-1 text-sm font-bold text-dim">
									{app.t('requirements-recommended')}
								</span>
							</div>
						{/if}
						{#each cards.requirements.rows as row (row.label + row.minimum)}
							<div class="flex gap-3">
								<span class="w-[130px] shrink-0 text-sm text-dim">{row.label}</span>
								<span class="flex-1 text-sm">{row.minimum}</span>
								{#if cards.requirements.both}
									<span class="flex-1 text-sm">{row.recommended}</span>
								{/if}
							</div>
						{/each}
					</div>
				</section>
			{/if}

			{#each cards?.failures ?? [] as failure (failure.label)}
				<section class="border border-line bg-surface p-4">
					<h2 class="mb-3 text-[11px] text-dim uppercase">{failure.label}</h2>
					<p class="mb-3 text-[13px] text-dim">{failure.value}</p>
					<button
						type="button"
						onclick={() => void retry()}
						class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
					>
						{app.t('try-again')}
					</button>
				</section>
			{/each}

			{#if game.sessions.length > 0}
				<section class="border border-line bg-surface p-4">
					<h2 class="mb-3 text-[11px] text-dim uppercase">{app.t('stats-game-title')}</h2>
					<div class="flex flex-wrap gap-6">
						{#each game.sessions as fact (fact.label)}
							<div>
								<p class="text-[11px] text-dim uppercase">{fact.label}</p>
								<p class="mt-1 text-lg font-bold">{fact.value}</p>
							</div>
						{/each}
					</div>
					{#if game.recent.length > 0}
						<ul class="mt-4 flex flex-col gap-1">
							{#each game.recent as row (row.label)}
								<li class="flex gap-4 text-[13px]">
									<span class="w-[110px] shrink-0 text-dim">{row.label}</span>
									<span>{row.value}</span>
								</li>
							{/each}
						</ul>
					{/if}
				</section>
			{/if}

			{#if game.prefix}
				<section class="border border-line bg-surface p-4">
					<h2 class="mb-3 text-[11px] text-dim uppercase">{app.t('prefix-title')}</h2>
					<p class="mb-3 text-[13px] text-dim">{app.t('prefix-about')}</p>
					<div class="flex flex-wrap items-center gap-2">
						<button
							type="button"
							disabled={!game.prefix.free}
							onclick={() => void runTool('wine-settings')}
							class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
						>
							{app.t('prefix-wine-settings')}
						</button>
						<button
							type="button"
							disabled={!game.prefix.free}
							onclick={() => void runTool('winetricks')}
							class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
						>
							winetricks
						</button>
						<button
							type="button"
							disabled={!game.prefix.free}
							onclick={() => void pickProgram()}
							class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
						>
							{app.t('prefix-run-program')}
						</button>
						{#if game.prefix.gog}
							<button
								type="button"
								disabled={!game.prefix.free}
								onclick={() => void runTool('gog-setup')}
								class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
							>
								{app.t('prefix-gog-setup')}
							</button>
						{/if}
						{#if game.prefix.folder}
							<button
								type="button"
								onclick={() => void prefixOpen(game!.prefix!.folder!)}
								class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
							>
								{app.t('files-open-folder')}
							</button>
						{/if}
					</div>

					{#if game.prefix.folder}
						<p class="mt-3 font-mono text-[13px] break-all">{game.prefix.folder}</p>
					{/if}

					<div class="mt-4 flex flex-wrap items-center gap-2">
						<input
							type="text"
							bind:value={prefixInput}
							placeholder={app.t('prefix-components-placeholder')}
							class="h-8 min-w-[240px] flex-1 border border-line bg-background px-2 text-[13px]"
							onkeydown={(event) => {
								if (event.key === 'Enter' && game!.prefix!.free) {
									event.preventDefault();
									void installComponents();
								}
							}}
						/>
						<button
							type="button"
							disabled={!game.prefix.free}
							onclick={() => void installComponents()}
							class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
						>
							{app.t('prefix-install')}
						</button>
					</div>

					{#if game.prefix.fixes.length > 0}
						<div class="mt-4 flex items-end justify-between gap-3">
							<div>
								<p class="text-[13px]">{app.t('prefix-known-fixes-title')}</p>
								<ul class="mt-1 flex flex-col gap-1">
									{#each game.prefix.fixes as fix (fix)}
										<li class="text-[13px] text-dim">{fix}</li>
									{/each}
								</ul>
							</div>
							<button
								type="button"
								disabled={!game.prefix.free}
								onclick={() => void runTool('known-fixes')}
								class="shrink-0 border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
							>
								{app.t('prefix-known-fixes')}
							</button>
						</div>
					{/if}

					{#if game.prefix.status || prefixNote}
						<p
							class="mt-3 text-[13px] {prefixNote || game.prefix.status?.tone === 2
								? 'text-bad'
								: game.prefix.status?.tone === 1
									? 'text-good'
									: 'text-dim'}"
						>
							{prefixNote ?? game.prefix.status?.text}
						</p>
					{/if}
				</section>
			{/if}

			{#if mods?.show}
				<section class="border border-line bg-surface p-4">
					<div class="mb-3 flex flex-wrap items-center justify-between gap-2">
						<h2 class="text-[11px] text-dim uppercase">{app.t('mods-title')}</h2>
						<div class="flex flex-wrap items-center gap-2">
							{#if mods.nexus}
								<button
									type="button"
									onclick={() => void findOnNexus()}
									class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
								>
									{app.t('mods-find-on-nexus')}
								</button>
								<button
									type="button"
									onclick={() => void search.openFor(id)}
									class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
								>
									{app.t('mods-search-nexus')}
								</button>
							{/if}
							<button
								type="button"
								disabled={!mods.free}
								onclick={() => void addMod()}
								class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
							>
								{app.t('mods-add')}
							</button>
						</div>
					</div>

					{#if mods.body_kind !== 2}
						<p class="text-[13px] {mods.body_kind === 1 ? 'text-bad' : 'text-dim'}">
							{mods.message}
						</p>
					{:else}
						<div class="flex flex-col gap-3">
							{#if downloads.length > 0}
								<div class="flex flex-col gap-3 border-b border-line pb-3">
									{#each downloads as download (download.id)}
										<div class="flex flex-col gap-1">
											<div class="flex items-center gap-3">
												<div class="min-w-0 flex-1">
													<p class="truncate text-[14px]">{download.name}</p>
													<p class="truncate text-[12px] text-dim">{download.words}</p>
												</div>
												<button
													type="button"
													disabled={download.stopping}
													onclick={() => void stopDownload(download.id)}
													class="border border-line px-2 py-0.5 text-[12px] uppercase hover:bg-raised disabled:opacity-50"
												>
													{download.stopping ? app.t('nexus-stopping') : app.t('nexus-stop')}
												</button>
											</div>
											<div class="h-1 bg-raised">
												<div
													class="h-full bg-accent-mark"
													style="width: {download.fraction * 100}%"
												></div>
											</div>
										</div>
									{/each}
								</div>
							{/if}
							{#if mods.naming}
								<div class="flex flex-wrap items-center gap-2">
									<span class="w-[90px] shrink-0 text-[13px] text-dim">
										{app.t('mods-profile')}
									</span>
									<input
										type="text"
										bind:value={profileName}
										placeholder={mods.naming.placeholder}
										class="h-8 min-w-[200px] flex-1 border border-line bg-background px-2 text-[13px]"
										onkeydown={(event) => {
											if (event.key === 'Enter' && mods!.naming!.ok) {
												event.preventDefault();
												void modsAct(() => modsProfileSave(id, profileName));
											}
										}}
									/>
									<button
										type="button"
										disabled={!mods.naming.ok}
										onclick={() => void modsAct(() => modsProfileSave(id, profileName))}
										class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
									>
										{app.t('key-save')}
									</button>
									<button
										type="button"
										onclick={() => void modsAct(() => modsProfileCancel(id))}
										class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
									>
										{app.t('cancel')}
									</button>
								</div>
							{:else}
								<div class="flex flex-wrap items-center gap-2">
									<span class="w-[90px] shrink-0 text-[13px] text-dim">
										{app.t('mods-profile')}
									</span>
									{#each mods.profiles as profile (profile.label)}
										<button
											type="button"
											disabled={!mods.free}
											aria-pressed={profile.chosen}
											onclick={() => void modsAct(() => modsProfileSwitch(id, profile.label))}
											class="border px-2 py-0.5 text-[13px] uppercase disabled:opacity-60 {profile.chosen
												? 'border-accent-mark text-accent-mark'
												: 'border-line text-foreground hover:bg-raised'}"
										>
											{profile.label}
										</button>
									{/each}
									<button
										type="button"
										disabled={!mods.free}
										onclick={() => void modsAct(() => modsProfileNaming(id, false))}
										class="border border-line px-2 py-0.5 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
									>
										{app.t('mods-profile-new')}
									</button>
									<button
										type="button"
										disabled={!mods.free}
										onclick={() => void modsAct(() => modsProfileNaming(id, true))}
										class="border border-line px-2 py-0.5 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
									>
										{app.t('mods-profile-rename-button')}
									</button>
									<button
										type="button"
										disabled={!mods.delete_profile_on}
										onclick={() => void modsAct(() => modsProfileDelete(id))}
										class="border px-2 py-0.5 text-[13px] uppercase disabled:opacity-60 {mods.delete_profile_asking
											? 'border-bad text-bad'
											: 'border-line text-foreground hover:bg-raised'}"
									>
										{mods.delete_profile_label}
									</button>
								</div>
							{/if}

							<p class="text-[13px] text-dim">{mods.place}</p>
							<p class="font-mono text-[13px] break-all">{mods.target}</p>

							{#if mods.mods.length === 0}
								<p class="text-[14px] text-dim">{app.t('mods-none')}</p>
							{:else}
								<ul class="flex flex-col gap-2">
									{#each mods.mods as mod, at (mod.id)}
										<li class="flex flex-col gap-0.5">
											<div class="flex flex-wrap items-center gap-2">
												<input
													type="checkbox"
													checked={mod.enabled}
													disabled={!mods.free}
													onchange={(event) =>
														void modsAct(() =>
															modsToggleMod(id, mod.id, event.currentTarget.checked)
														)}
												/>
												<span class="min-w-0 flex-1 truncate text-[14px]">{mod.name}</span>
												{#if mod.update}
													<button
														type="button"
														disabled={!mods.free}
														onclick={() => void nexusAct(() => nexusUpdate(id, at))}
														class="border border-accent-mark px-2 text-[13px] text-accent-mark uppercase hover:bg-raised disabled:opacity-60"
													>
														{mod.update}
													</button>
												{/if}
												{#if mod.options}
													<button
														type="button"
														disabled={!mods.free}
														onclick={() => void modsAct(() => modsReinstall(id, mod.id))}
														class="border border-line px-2 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
													>
														{mod.options}
													</button>
												{/if}
												<button
													type="button"
													disabled={!mods.free || !mod.up}
													onclick={() => void modsAct(() => modsMoveMod(id, mod.id, true))}
													class="border border-line px-2 text-[13px] hover:bg-raised disabled:opacity-40"
												>
													↑
												</button>
												<button
													type="button"
													disabled={!mods.free || !mod.down}
													onclick={() => void modsAct(() => modsMoveMod(id, mod.id, false))}
													class="border border-line px-2 text-[13px] hover:bg-raised disabled:opacity-40"
												>
													↓
												</button>
												<button
													type="button"
													disabled={!mods.free}
													onclick={() => void modsAct(() => modsRemove(id, mod.id))}
													class="border px-2 text-[13px] uppercase disabled:opacity-60 {mod.asking
														? 'border-bad text-bad'
														: 'border-line text-foreground hover:bg-raised'}"
												>
													{mod.delete_label}
												</button>
											</div>
											<p class="pl-7 text-[12px] text-dim">{mod.facts}</p>
										</li>
									{/each}
								</ul>
							{/if}

							{#if mods.plugins}
								<div class="flex flex-col gap-1">
									<p class="text-[13px] font-bold uppercase">{app.t('plugins-title')}</p>
									<p class="text-[13px] {mods.plugins.note_bad ? 'text-bad' : 'text-dim'}">
										{mods.plugins.note}
									</p>
									{#if mods.plugins.file}
										<p class="font-mono text-[12px] break-all text-dim">{mods.plugins.file}</p>
									{/if}
									{#if mods.plugins.rows.length === 0}
										<p class="text-[13px] text-dim">{app.t('plugins-none')}</p>
									{/if}
									{#each mods.plugins.rows as plugin (plugin.name)}
										<div class="flex flex-col gap-0.5">
											<div class="flex flex-wrap items-center gap-2">
												<input
													type="checkbox"
													checked={plugin.enabled}
													disabled={!mods.free || plugin.locked}
													onchange={(event) =>
														void modsAct(() =>
															modsTogglePlugin(id, plugin.name, event.currentTarget.checked)
														)}
												/>
												<span class="min-w-0 flex-1 truncate text-[14px]">{plugin.name}</span>
												<button
													type="button"
													disabled={!mods.free || !plugin.up}
													onclick={() => void modsAct(() => modsMovePlugin(id, plugin.name, true))}
													class="border border-line px-2 text-[13px] hover:bg-raised disabled:opacity-40"
												>
													↑
												</button>
												<button
													type="button"
													disabled={!mods.free || !plugin.down}
													onclick={() => void modsAct(() => modsMovePlugin(id, plugin.name, false))}
													class="border border-line px-2 text-[13px] hover:bg-raised disabled:opacity-40"
												>
													↓
												</button>
											</div>
											<p class="pl-7 text-[12px] text-dim">{plugin.facts}</p>
											{#if plugin.problems}
												<p class="pl-7 text-[12px] text-bad">{plugin.problems}</p>
											{/if}
										</div>
									{/each}
								</div>
							{/if}

							{#if mods.overwrite}
								<div class="flex flex-wrap items-center gap-2">
									<span class="min-w-0 flex-1 text-[13px] text-dim">{mods.overwrite}</span>
									<button
										type="button"
										disabled={!mods.free}
										onclick={() => void modsAct(() => modsClearOverwrite(id))}
										class="border px-2 py-0.5 text-[13px] uppercase disabled:opacity-60 {mods.overwrite_asking
											? 'border-bad text-bad'
											: 'border-line text-foreground hover:bg-raised'}"
									>
										{mods.overwrite_label}
									</button>
								</div>
							{/if}

							<div class="flex flex-wrap items-center gap-2">
								<span class="w-[90px] shrink-0 text-[13px] text-dim">{app.t('mods-folder')}</span>
								<input
									type="text"
									bind:value={folderInput}
									readonly={!mods.folder_editable}
									placeholder={app.t('mods-folder-placeholder')}
									class="h-8 min-w-[240px] flex-1 border border-line bg-background px-2 text-[13px]"
									onkeydown={(event) => {
										if (event.key === 'Enter') {
											event.preventDefault();
											void modsAct(() => modsFolderSave(id, folderInput));
										}
									}}
								/>
								{#if mods.folder_save}
									<button
										type="button"
										disabled={!mods.free}
										onclick={() => void modsAct(() => modsFolderSave(id, folderInput))}
										class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
									>
										{app.t('key-save')}
									</button>
								{/if}
								{#if mods.folder_known}
									<button
										type="button"
										disabled={!mods.free}
										onclick={() => void modsAct(() => modsFolderKnown(id))}
										class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
									>
										{app.t('mods-folder-known')}
									</button>
								{/if}
							</div>

							<div class="flex flex-wrap items-center gap-2">
								<span
									class="min-w-0 flex-1 text-[13px] {mods.state_tone === 1
										? 'text-good'
										: mods.state_tone === 2
											? 'text-bad'
											: 'text-dim'}"
								>
									{mods.state}
								</span>
								<button
									type="button"
									disabled={!mods.can_restore}
									onclick={() => void modsAct(() => modsApply(id, false))}
									class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
								>
									{app.t('mods-back-to-vanilla')}
								</button>
								<button
									type="button"
									disabled={!mods.can_apply}
									onclick={() => void modsAct(() => modsApply(id, true))}
									class="border border-accent-mark bg-accent-mark px-3 py-1 text-[13px] font-bold text-background uppercase disabled:opacity-60"
								>
									{app.t('mods-apply')}
								</button>
							</div>
						</div>
					{/if}

					{#if mods.busy}
						<p class="mt-3 text-[13px] text-dim">{mods.busy}</p>
					{/if}
					{#if modsNote ?? mods.note}
						<p class="mt-1 text-[13px] text-dim">{modsNote ?? mods.note}</p>
					{/if}
				</section>
			{/if}

			{#if cards && cards.changelog.lines.length > 0}
				<section class="border border-line bg-surface p-4">
					<h2 class="mb-3 text-[11px] text-dim uppercase">{app.t('changelog-title')}</h2>
					<div class="flex flex-col gap-1">
						{#each cards.changelog.lines as line, index (index)}
							{#if line.kind === 4}
								<span class="h-2"></span>
							{:else}
								<p
									class="text-[14px] whitespace-pre-line {line.kind === 0
										? 'text-[15px] font-bold'
										: line.kind === 1
											? 'font-bold text-dim'
											: ''}"
								>
									{line.kind === 3 ? `•  ${line.text}` : line.text}
								</p>
							{/if}
						{/each}
					</div>
					{#if cards.changelog.more}
						<button
							type="button"
							onclick={() => void moreChangelog()}
							class="mt-3 border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
						>
							{cards.changelog.more}
						</button>
					{/if}
				</section>
			{/if}

			{#if game.dlc.rows.length > 0}
				<section class="flex flex-col gap-3 border border-line bg-surface p-4">
					<h2 class="text-[11px] text-dim uppercase">{app.t('dlc-title')}</h2>
					<div class="flex items-center gap-3">
						<p class="flex-1 text-[13px] text-dim">{game.dlc.count}</p>
						{#if game.dlc.all}
							<button
								type="button"
								disabled={!game.dlc.all_on}
								onclick={() => void act(() => gameDlcAll(id))}
								class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-50"
							>
								{game.dlc.all}
							</button>
						{/if}
					</div>
					<ul class="flex flex-col gap-2">
						{#each game.dlc.rows as row (row.id)}
							<li class="flex items-center gap-3">
								<div class="min-w-0 flex-1">
									<p class="text-sm">{row.title}</p>
									{#if row.about}
										<p class="truncate text-xs text-dim">{row.about}</p>
									{/if}
								</div>
								{#if row.status}
									<p class="text-[13px] {row.status_tone === 1 ? 'text-good' : 'text-dim'}">
										{row.status}
									</p>
								{/if}
								{#if row.label}
									<button
										type="button"
										disabled={!row.enabled}
										onclick={() => void act(() => gameDlc(id, row.id))}
										class="border px-3 py-1 text-[13px] uppercase disabled:opacity-50 {row.danger
											? 'border-bad text-bad'
											: 'border-line text-foreground hover:bg-raised'}"
									>
										{row.label}
									</button>
								{/if}
							</li>
						{/each}
					</ul>
					{#if game.dlc.note}
						<p class="text-xs whitespace-pre-line text-dim">{game.dlc.note}</p>
					{/if}
				</section>
			{/if}

			<section class="border border-line bg-surface p-4">
				<h2 class="mb-3 text-[11px] text-dim uppercase">{app.t('game-details')}</h2>
				<dl class="flex flex-col gap-1">
					{#each detailRows as row (row.label)}
						<div class="flex gap-4 text-[13px]">
							<dt class="w-[160px] shrink-0 text-dim">{row.label}</dt>
							{#if row.url}
								<dd class="break-all">
									<button
										type="button"
										onclick={() => void openLink(row.url!)}
										class="text-accent-mark hover:underline"
									>
										{row.value}
									</button>
								</dd>
							{:else}
								<dd
									class="break-all {row.tone === 1
										? 'text-good'
										: row.tone === 2
											? 'text-bad'
											: row.tone === 3
												? 'text-dim'
												: ''}"
								>
									{row.value}
								</dd>
							{/if}
						</div>
					{/each}
				</dl>
			</section>

			{#if cards && cards.steam_app.line}
				<section class="border border-line bg-surface p-4">
					<h2 class="mb-3 text-[11px] text-dim uppercase">{app.t('game-technical')}</h2>
					<div class="flex flex-col gap-2">
						<span class="text-[11px] text-dim uppercase">{app.t('steam-app-label')}</span>
						<p class="text-[13px] font-bold">{cards.steam_app.line}</p>
						<div class="flex flex-wrap items-center gap-2">
							<input
								type="text"
								bind:value={appInput}
								placeholder={app.t('steam-app-placeholder')}
								class="h-8 w-[160px] border border-line bg-background px-2 text-[13px]"
								onkeydown={(event) => {
									if (event.key === 'Enter') {
										event.preventDefault();
										void saveApp(appInput);
									}
								}}
							/>
							<button
								type="button"
								onclick={() => void saveApp(appInput)}
								class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
							>
								{app.t('steam-app-save')}
							</button>
							{#if cards.steam_app.clearable}
								<button
									type="button"
									onclick={() => void saveApp(null)}
									class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
								>
									{app.t('steam-app-clear')}
								</button>
							{/if}
						</div>
						{#if appNote}
							<p class="text-[13px] text-dim">{appNote}</p>
						{/if}
					</div>
				</section>
			{/if}
		</div>
	</div>

	{#if editing}
		<LocalForm
			{id}
			onclose={() => (editing = false)}
			onsaved={() => {
				editing = false;
				void load();
			}}
		/>
	{/if}

	{#if options.open}
		<OptionsSheet />
	{/if}

	{#if artwork.open}
		<ArtSheet />
	{/if}

	{#if search.open}
		<SearchSheet />
	{/if}
{/if}
