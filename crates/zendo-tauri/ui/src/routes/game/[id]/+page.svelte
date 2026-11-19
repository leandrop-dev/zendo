<script lang="ts">
	import { convertFileSrc } from '@tauri-apps/api/core';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { listen } from '@tauri-apps/api/event';
	import { onMount } from 'svelte';
	import {
		cardsGet,
		cardsMoreAchievements,
		cardsRetry,
		gameGet,
		gameLaunch,
		gameSetHidden,
		gameSetPinned,
		gameSetStatus,
		gameStop,
		inTauri,
		localRemove,
		openLink,
		prefixInstall,
		prefixOpen,
		prefixPickProgram,
		prefixRun,
		prefixWant,
		steamAppSet,
		type GameCards,
		type GamePage,
		type Pill
	} from '#lib/api.js';
	import LocalForm from '#lib/components/local-form.svelte';
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
		...(cards?.steam ?? [])
	]);
	// Every file of the cards is in Zendo's cache; the asset protocol serves it.
	function asset(path: string | null): string {
		return path !== null && inTauri() ? convertFileSrc(path) : '';
	}

	async function load(): Promise<void> {
		if (!inTauri() || !id) {
			return;
		}
		try {
			game = await gameGet(id);
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

	function key(event: KeyboardEvent): void {
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

	onMount(() => {
		void load();
		void loadCards();
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
		// A prefix tool ended, or the known fixes' list was read again.
		const prefixChanged = listen<string>('prefix_changed', (event) => {
			if (event.payload === id) {
				void load();
			}
		});
		return () => {
			window.removeEventListener('keydown', key);
			void runs.then((stop) => stop());
			void lines.then((stop) => stop());
			void arrived.then((stop) => stop());
			void prefixChanged.then((stop) => stop());
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

			{#if cards && cards.shots.length > 0}
				<section class="border border-line bg-surface p-4">
					<h2 class="mb-3 text-[11px] text-dim uppercase">{app.t('screenshots-title')}</h2>
					<div class="flex gap-3 overflow-x-auto">
						{#each cards.shots as shot (shot.thumb)}
							<img
								src={asset(shot.thumb)}
								alt=""
								loading="lazy"
								class="h-[150px] shrink-0 rounded-[2px] object-cover"
							/>
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

			{#if game.dlc.length > 0}
				<section class="border border-line bg-surface p-4">
					<h2 class="mb-3 text-[11px] text-dim uppercase">{app.t('dlc-title')}</h2>
					<ul class="flex flex-col gap-1">
						{#each game.dlc as dlc (dlc.title)}
							<li class="text-[13px]">{dlc.title}</li>
						{/each}
					</ul>
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
								<dd class="break-all">{row.value}</dd>
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
{/if}
