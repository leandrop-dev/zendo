<script lang="ts">
	import { listen } from '@tauri-apps/api/event';
	import { onMount } from 'svelte';
	import {
		inTauri,
		settingsBuildChosen,
		settingsCopyReport,
		settingsDefaultChosen,
		settingsDownloadCovers,
		settingsEngineAct,
		settingsEngineRemove,
		settingsEnginesCheck,
		settingsEnginesStop,
		settingsGet,
		settingsKeyCheck,
		settingsKeyEdited,
		settingsKeyGet,
		settingsKeySave,
		settingsKeyToggleShown,
		settingsLauncherAct,
		settingsLaunchersStop,
		settingsNexusBecomeHandler,
		settingsNexusKeyCheck,
		settingsNexusKeyEdited,
		settingsNexusKeyGet,
		settingsNexusKeySave,
		settingsNexusKeyToggleShown,
		settingsOpen,
		settingsOverlayDoInstall,
		settingsOverlayDoRemove,
		settingsProtonRefresh,
		settingsProtonRemove,
		settingsProtonShowOlder,
		settingsProtonStop,
		settingsReleaseDownload,
		settingsRuntimeAct,
		settingsRuntimeRemove,
		settingsSetAutoUpdate,
		settingsSetCovers,
		settingsSetDiscord,
		settingsSetLanguage,
		settingsSetPathFirst,
		settingsSetTheme,
		settingsSetTray,
		settingsSetWorkers,
		type SettingsPage
	} from '#lib/api.js';
	import DownloadBar from '#lib/components/settings/download-bar.svelte';
	import SetList from '#lib/components/settings/set-list.svelte';
	import { app } from '#lib/state.svelte.js';

	// The Settings page's General, Library and Diagnostics groups: one at a
	// time, as the Slint window's are. Keys, Launchers, Engines, Proton and
	// Discord's presence are the next part of this slice.
	const GROUPS = [
		{ key: 'general', label: 'settings-group-general' },
		{ key: 'keys', label: 'settings-group-keys' },
		{ key: 'launchers', label: 'settings-group-launchers' },
		{ key: 'engines', label: 'settings-group-engines' },
		{ key: 'proton', label: 'settings-group-proton' },
		{ key: 'library', label: 'settings-group-library' },
		{ key: 'diagnostics', label: 'settings-group-diagnostics' }
	] as const;

	let group = $state<(typeof GROUPS)[number]['key']>('general');
	let settings = $state<SettingsPage | null>(null);
	let error = $state<string | null>(null);
	// The key fields show what is typed; an answer from a check or a save
	// must not clear them, so they are set only on the first load.
	let steamgriddbValue = $state('');
	let nexusValue = $state('');

	async function load(): Promise<void> {
		if (!inTauri()) {
			return;
		}
		try {
			const page = await settingsGet();
			if (!settings) {
				steamgriddbValue = page.steamgriddb.value;
				nexusValue = page.nexus.value;
			}
			settings = page;
		} catch (err) {
			error = String(err);
		}
	}

	async function act(action: () => Promise<SettingsPage>): Promise<void> {
		try {
			settings = await action();
		} catch (err) {
			error = String(err);
		}
	}

	function rowTone(tone: number): string {
		return tone === 2 ? 'text-bad' : tone === 1 ? 'text-good' : tone === 3 ? 'text-dim' : '';
	}

	function onSteamgriddbInput(event: Event): void {
		steamgriddbValue = (event.currentTarget as HTMLInputElement).value;
		void act(() => settingsKeyEdited(steamgriddbValue));
	}

	function onNexusInput(event: Event): void {
		nexusValue = (event.currentTarget as HTMLInputElement).value;
		void act(() => settingsNexusKeyEdited(nexusValue));
	}

	onMount(() => {
		void load();
		const changed = listen('settings_changed', () => void load());
		const loaded = listen('library_loaded', () => void load());
		return () => {
			void changed.then((stop) => stop());
			void loaded.then((stop) => stop());
		};
	});
</script>

<svelte:head>
	<title>Settings · Zendo</title>
</svelte:head>

{#if error}
	<p class="p-8 text-sm text-bad">{error}</p>
{:else if !settings}
	<p class="p-8 text-sm text-dim">{app.t('library-looking')}</p>
{:else}
	<div class="flex h-full min-h-0 flex-col">
		<div class="flex flex-wrap items-center gap-2 px-6 pt-6 pb-1">
			{#each GROUPS as chip (chip.key)}
				<button
					type="button"
					aria-pressed={group === chip.key}
					onclick={() => (group = chip.key)}
					class="border px-2 py-1 text-[13px] uppercase {group === chip.key
						? 'border-accent-mark text-accent-mark'
						: 'border-line text-foreground hover:bg-raised'}"
				>
					{app.t(chip.label)}
				</button>
			{/each}
		</div>

		<div class="flex h-full min-h-0 flex-col gap-4 overflow-y-auto p-6">
			{#if group === 'general'}
				<section class="border border-line bg-surface p-4">
					<h2 class="text-[11px] text-dim uppercase">{app.t('language-title')}</h2>
					<p class="mt-1 mb-3 text-[13px] text-dim">{app.t('language-about')}</p>
					<div class="flex flex-wrap gap-2">
						{#each settings.languages as language (language.label)}
							<button
								type="button"
								aria-pressed={language.chosen}
								onclick={() => void act(() => settingsSetLanguage(language.tag))}
								class="border px-2 py-1 text-[13px] uppercase {language.chosen
									? 'border-accent-mark text-accent-mark'
									: 'border-line text-foreground hover:bg-raised'}"
							>
								{language.label}
							</button>
						{/each}
					</div>
				</section>

				<section class="border border-line bg-surface p-4">
					<h2 class="text-[11px] text-dim uppercase">{app.t('theme-title')}</h2>
					<p class="mt-1 mb-3 text-[13px] text-dim">{settings.theme_about}</p>
					<div class="flex flex-wrap gap-2">
						{#each settings.themes as theme (theme.tag)}
							<button
								type="button"
								aria-pressed={theme.chosen}
								onclick={() => void act(() => settingsSetTheme(theme.tag))}
								class="border px-2 py-1 text-[13px] uppercase {theme.chosen
									? 'border-accent-mark text-accent-mark'
									: 'border-line text-foreground hover:bg-raised'}"
							>
								{theme.label}
							</button>
						{/each}
					</div>
					{#if settings.theme_note}
						<p class="mt-3 text-[13px] {rowTone(settings.theme_note.tone)}">
							{settings.theme_note.value}
						</p>
					{/if}
					{#each settings.theme_problems as problem (problem)}
						<p class="mt-1 text-[13px] text-bad">{problem}</p>
					{/each}
				</section>

				<section class="border border-line bg-surface p-4">
					<h2 class="text-[11px] text-dim uppercase">{app.t('window-title')}</h2>
					<p class="mt-1 mb-3 text-[13px] text-dim">{app.t('window-about')}</p>
					<label class="flex items-center gap-2 text-[13px]">
						<input
							type="checkbox"
							checked={settings.tray}
							onchange={(event) => void act(() => settingsSetTray(event.currentTarget.checked))}
						/>
						{app.t('minimize-to-tray')}
					</label>
				</section>

				<section class="border border-line bg-surface p-4">
					<h2 class="text-[11px] text-dim uppercase">{app.t('discord-title')}</h2>
					<p class="mt-1 mb-3 text-[13px] text-dim">{app.t('discord-about')}</p>
					<label class="flex items-center gap-2 text-[13px]">
						<input
							type="checkbox"
							checked={settings.discord}
							onchange={(event) => void act(() => settingsSetDiscord(event.currentTarget.checked))}
						/>
						{app.t('discord-presence')}
					</label>
				</section>
			{:else if group === 'keys'}
				<section class="border border-line bg-surface p-4">
					<h2 class="text-[11px] text-dim uppercase">SteamGridDB</h2>
					<p class="mt-1 mb-3 text-[13px] text-dim">{app.t('steamgriddb-about')}</p>
					<div class="flex flex-wrap gap-2">
						<input
							type={settings.steamgriddb.shown ? 'text' : 'password'}
							value={steamgriddbValue}
							oninput={onSteamgriddbInput}
							placeholder={app.t('steamgriddb-paste')}
							class="min-w-0 flex-1 border border-line bg-background px-2 py-1 text-[13px]"
						/>
						<button
							type="button"
							onclick={() => void act(settingsKeyToggleShown)}
							class="border border-line px-2 py-1 text-[13px] uppercase hover:bg-raised"
						>
							{settings.steamgriddb.shown ? app.t('key-hide') : app.t('key-show')}
						</button>
						<button
							type="button"
							disabled={settings.steamgriddb.checking || steamgriddbValue === ''}
							onclick={() => void act(settingsKeyCheck)}
							class="border border-line px-2 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
						>
							{settings.steamgriddb.checking ? app.t('key-checking') : app.t('key-check')}
						</button>
						<button
							type="button"
							disabled={!settings.steamgriddb.changed}
							onclick={() => void act(settingsKeySave)}
							class="border border-accent-mark bg-accent-mark px-3 py-1 text-[13px] font-bold text-background uppercase disabled:opacity-60"
						>
							{app.t('key-save')}
						</button>
					</div>
					{#if settings.steamgriddb.note}
						<p class="mt-3 text-[13px] {rowTone(settings.steamgriddb.note.tone)}">
							{settings.steamgriddb.note.value}
						</p>
					{/if}
					{#if settings.steamgriddb.kept}
						<p class="mt-1 text-[13px] {settings.steamgriddb.kept_bad ? 'text-bad' : 'text-dim'}">
							{settings.steamgriddb.kept}
						</p>
					{/if}
					<button
						type="button"
						onclick={() => void settingsKeyGet()}
						class="mt-3 text-[13px] underline hover:text-accent-mark"
					>
						{app.t('steamgriddb-get-key')}
					</button>
				</section>

				<section class="border border-line bg-surface p-4">
					<h2 class="text-[11px] text-dim uppercase">Nexus Mods</h2>
					<p class="mt-1 mb-3 text-[13px] text-dim">{app.t('nexus-about')}</p>
					<div class="flex flex-wrap gap-2">
						<input
							type={settings.nexus.shown ? 'text' : 'password'}
							value={nexusValue}
							oninput={onNexusInput}
							placeholder={app.t('nexus-paste')}
							class="min-w-0 flex-1 border border-line bg-background px-2 py-1 text-[13px]"
						/>
						<button
							type="button"
							onclick={() => void act(settingsNexusKeyToggleShown)}
							class="border border-line px-2 py-1 text-[13px] uppercase hover:bg-raised"
						>
							{settings.nexus.shown ? app.t('key-hide') : app.t('key-show')}
						</button>
						<button
							type="button"
							disabled={settings.nexus.checking || nexusValue === ''}
							onclick={() => void act(settingsNexusKeyCheck)}
							class="border border-line px-2 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
						>
							{settings.nexus.checking ? app.t('key-checking') : app.t('key-check')}
						</button>
						<button
							type="button"
							disabled={!settings.nexus.changed}
							onclick={() => void act(settingsNexusKeySave)}
							class="border border-accent-mark bg-accent-mark px-3 py-1 text-[13px] font-bold text-background uppercase disabled:opacity-60"
						>
							{app.t('key-save')}
						</button>
					</div>
					{#if settings.nexus.note}
						<p class="mt-3 text-[13px] {rowTone(settings.nexus.note.tone)}">
							{settings.nexus.note.value}
						</p>
					{/if}
					{#if settings.nexus.kept}
						<p class="mt-1 text-[13px] {settings.nexus.kept_bad ? 'text-bad' : 'text-dim'}">
							{settings.nexus.kept}
						</p>
					{/if}
					<button
						type="button"
						onclick={() => void settingsNexusKeyGet()}
						class="mt-3 text-[13px] underline hover:text-accent-mark"
					>
						{app.t('nexus-get-key')}
					</button>
					{#if settings.nexus.handler_line}
						<p class="mt-3 text-[13px] {settings.nexus.handler_good ? 'text-good' : 'text-dim'}">
							{settings.nexus.handler_line}
						</p>
					{/if}
					{#if settings.nexus.handler_can}
						<div class="mt-2 flex justify-end">
							<button
								type="button"
								onclick={() => void act(settingsNexusBecomeHandler)}
								class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
							>
								{app.t('nexus-become-handler')}
							</button>
						</div>
					{/if}
				</section>
			{:else if group === 'launchers'}
				<section class="border border-line bg-surface p-4">
					<h2 class="text-[11px] text-dim uppercase">{app.t('launchers-title')}</h2>
					<p class="mt-1 mb-3 text-[13px] text-dim">{app.t('launchers-about')}</p>
					<SetList
						rows={settings.launchers}
						onact={(index) => void act(() => settingsLauncherAct(index))}
					/>
					<DownloadBar
						bar={settings.launchers_bar}
						onstop={() => void act(settingsLaunchersStop)}
					/>
				</section>
			{:else if group === 'engines'}
				<section class="border border-line bg-surface p-4">
					<h2 class="text-[11px] text-dim uppercase">{app.t('engines-title')}</h2>
					<p class="mt-1 mb-3 text-[13px] text-dim">{app.t('engines-about')}</p>
					<label class="mb-3 flex items-center gap-2 text-[13px]">
						<input
							type="checkbox"
							checked={settings.path_first}
							onchange={(event) =>
								void act(() => settingsSetPathFirst(event.currentTarget.checked))}
						/>
						{app.t('engines-path-first')}
					</label>
					<div class="mb-3">
						<button
							type="button"
							onclick={() => void act(settingsEnginesCheck)}
							class="border border-line px-2 py-1 text-[13px] uppercase hover:bg-raised"
						>
							{app.t('protons-check-again')}
						</button>
					</div>
					<SetList
						rows={settings.engines}
						onact={(index) => void act(() => settingsEngineAct(index))}
						onremove={(index) => void act(() => settingsEngineRemove(index))}
					/>
					<DownloadBar bar={settings.engines_bar} onstop={() => void act(settingsEnginesStop)} />
					{#if settings.engines_note}
						<p class="mt-3 text-[13px] {rowTone(settings.engines_note.tone)}">
							{settings.engines_note.value}
						</p>
					{/if}
				</section>

				<section class="border border-line bg-surface p-4">
					<h2 class="text-[11px] text-dim uppercase">{app.t('runtimes-title')}</h2>
					<p class="mt-1 mb-3 text-[13px] text-dim">{app.t('runtimes-about')}</p>
					<SetList
						rows={settings.runtimes}
						onact={(index) => void act(() => settingsRuntimeAct(index))}
						onremove={(index) => void act(() => settingsRuntimeRemove(index))}
					/>
					{#if settings.runtimes_note}
						<p class="mt-3 text-[13px] {rowTone(settings.runtimes_note.tone)}">
							{settings.runtimes_note.value}
						</p>
					{/if}
				</section>

				<section class="border border-line bg-surface p-4">
					<h2 class="text-[11px] text-dim uppercase">{app.t('overlay-title')}</h2>
					<p class="mt-1 mb-3 text-[13px] text-dim">{app.t('overlay-about')}</p>
					<p class="text-[13px] text-dim">{settings.overlay.status}</p>
					<div class="mt-2 flex gap-2">
						<button
							type="button"
							disabled={!settings.overlay.install_on}
							onclick={() => void act(settingsOverlayDoInstall)}
							class="border border-line px-2 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
						>
							{settings.overlay.install}
						</button>
						{#if settings.overlay.remove}
							<button
								type="button"
								disabled={!settings.overlay.remove_on}
								onclick={() => void act(settingsOverlayDoRemove)}
								class="border border-bad px-2 py-1 text-[13px] text-bad uppercase disabled:opacity-60"
							>
								{settings.overlay.remove}
							</button>
						{/if}
					</div>
					{#if settings.overlay.note}
						<p class="mt-3 text-[13px] {rowTone(settings.overlay.note.tone)}">
							{settings.overlay.note.value}
						</p>
					{/if}
				</section>
			{:else if group === 'proton'}
				<section class="border border-line bg-surface p-4">
					<h2 class="text-[11px] text-dim uppercase">Proton</h2>
					<p class="mt-1 mb-3 text-[13px] text-dim">{app.t('protons-about')}</p>
					<p class="text-[13px] font-bold">{app.t('protons-default')}</p>
					<p class="mt-1 mb-2 text-[13px] text-dim">{app.t('protons-default-about')}</p>
					<div class="flex flex-wrap gap-2">
						{#each settings.proton.defaults as chip (chip.name ?? 'auto')}
							<button
								type="button"
								aria-pressed={chip.chosen}
								onclick={() => void act(() => settingsDefaultChosen(chip.name))}
								class="border px-2 py-1 text-[13px] uppercase {chip.chosen
									? 'border-accent-mark text-accent-mark'
									: 'border-line text-foreground hover:bg-raised'}"
							>
								{chip.label}
							</button>
						{/each}
					</div>

					<p class="mt-4 text-[13px] font-bold">{app.t('protons-here')}</p>
					{#if settings.proton.installed_none}
						<p class="mt-1 text-[13px] text-dim">{settings.proton.installed_none}</p>
					{/if}
					<div class="mt-2">
						<SetList
							rows={settings.proton.installed}
							onremove={(index) => void act(() => settingsProtonRemove(index))}
						/>
					</div>

					<p class="mt-4 text-[13px] font-bold">{settings.proton.releases_title}</p>
					<div class="mt-2 flex flex-wrap gap-2">
						{#each settings.proton.builds as chip, index (chip.label)}
							<button
								type="button"
								aria-pressed={chip.chosen}
								onclick={() => void act(() => settingsBuildChosen(index))}
								class="border px-2 py-1 text-[13px] uppercase {chip.chosen
									? 'border-accent-mark text-accent-mark'
									: 'border-line text-foreground hover:bg-raised'}"
							>
								{chip.label}
							</button>
						{/each}
					</div>
					<div class="mt-2">
						<button
							type="button"
							disabled={!settings.proton.refresh_on}
							onclick={() => void act(settingsProtonRefresh)}
							class="border border-line px-2 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
						>
							{app.t('protons-check-again')}
						</button>
					</div>
					<div class="mt-2">
						<SetList
							rows={settings.proton.releases}
							onact={(index) => void act(() => settingsReleaseDownload(index))}
						/>
					</div>
					{#if settings.proton.line}
						<p class="mt-3 text-[13px] {rowTone(settings.proton.line.tone)}">
							{settings.proton.line.value}
						</p>
					{/if}
					{#if settings.proton.older}
						<div class="mt-2">
							<button
								type="button"
								onclick={() => void act(settingsProtonShowOlder)}
								class="border border-line px-2 py-1 text-[13px] uppercase hover:bg-raised"
							>
								{app.t('protons-show-older')}
							</button>
						</div>
					{/if}
					<DownloadBar bar={settings.proton.bar} onstop={() => void act(settingsProtonStop)} />
					{#if settings.proton.note}
						<p class="mt-3 text-[13px] {rowTone(settings.proton.note.tone)}">
							{settings.proton.note.value}
						</p>
					{/if}
				</section>
			{:else if group === 'library'}
				<section class="border border-line bg-surface p-4">
					<h2 class="text-[11px] text-dim uppercase">{app.t('covers-title')}</h2>
					<p class="mt-1 mb-3 text-[13px] text-dim">{app.t('covers-about')}</p>
					<label class="flex items-center gap-2 text-[13px]">
						<input
							type="checkbox"
							checked={settings.covers}
							onchange={(event) => void act(() => settingsSetCovers(event.currentTarget.checked))}
						/>
						{app.t('covers-download')}
					</label>
					<button
						type="button"
						disabled={!settings.covers_now}
						onclick={() => void act(() => settingsDownloadCovers())}
						class="mt-3 border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
					>
						{app.t('covers-download-now')}
					</button>
				</section>

				<section class="border border-line bg-surface p-4">
					<h2 class="text-[11px] text-dim uppercase">{app.t('downloads-settings-title')}</h2>
					<p class="mt-1 mb-3 text-[13px] text-dim">{app.t('downloads-settings-about')}</p>
					<label class="mb-3 flex items-center gap-2 text-[13px]">
						<input
							type="checkbox"
							checked={settings.auto_update}
							onchange={(event) =>
								void act(() => settingsSetAutoUpdate(event.currentTarget.checked))}
						/>
						{app.t('auto-update')}
					</label>
					<p class="mb-2 text-[13px]">{app.t('workers-title')}</p>
					<div class="flex flex-wrap gap-2">
						{#each settings.workers as worker (worker.label)}
							<button
								type="button"
								aria-pressed={worker.chosen}
								onclick={() => void act(() => settingsSetWorkers(worker.count))}
								class="border px-2 py-1 text-[13px] uppercase {worker.chosen
									? 'border-accent-mark text-accent-mark'
									: 'border-line text-foreground hover:bg-raised'}"
							>
								{worker.label}
							</button>
						{/each}
					</div>
					<p class="mt-2 text-[13px] text-dim">{app.t('workers-about')}</p>
				</section>

				<section class="border border-line bg-surface p-4">
					<h2 class="text-[11px] text-dim uppercase">{app.t('sources-title')}</h2>
					<p class="mt-1 mb-3 text-[13px] text-dim">{app.t('sources-about')}</p>
					<dl class="flex flex-col gap-1">
						{#each settings.sources as row (row.label + row.value)}
							<div class="flex gap-4 text-[13px]">
								<dt class="w-[220px] shrink-0">{row.label}</dt>
								<dd class={rowTone(row.tone)}>{row.value}</dd>
							</div>
						{/each}
					</dl>
				</section>
			{:else}
				<section class="border border-line bg-surface p-4">
					<h2 class="text-[11px] text-dim uppercase">{app.t('files-title')}</h2>
					<p class="mt-1 mb-3 text-[13px] text-dim">{app.t('files-about')}</p>
					<div class="flex flex-col gap-1">
						{#each settings.files as file (file.label)}
							<div class="flex items-start gap-3 text-[13px]">
								<span class="w-[160px] shrink-0 text-dim">{file.label}</span>
								<span class="min-w-0 flex-1 break-all">{file.value}</span>
								<button
									type="button"
									disabled={!file.open}
									onclick={() => void settingsOpen(file.open!)}
									class="shrink-0 border border-line px-2 py-0.5 text-[13px] uppercase hover:bg-raised disabled:opacity-40"
								>
									{app.t('files-open-folder')}
								</button>
							</div>
						{/each}
					</div>
				</section>

				<section class="border border-line bg-surface p-4">
					<h2 class="text-[11px] text-dim uppercase">{app.t('report-title')}</h2>
					<p class="mt-1 mb-3 text-[13px] text-dim">{app.t('report-about')}</p>
					<div class="flex flex-wrap gap-2">
						<button
							type="button"
							disabled={settings.report_busy}
							onclick={() => void act(() => settingsCopyReport())}
							class="border border-accent-mark bg-accent-mark px-3 py-1 text-[13px] font-bold text-background uppercase disabled:opacity-60"
						>
							{settings.report_busy ? app.t('report-gathering') : app.t('report-copy')}
						</button>
						<button
							type="button"
							disabled={!settings.logs}
							onclick={() => void settingsOpen(settings!.logs!)}
							class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
						>
							{app.t('report-open-logs')}
						</button>
					</div>
					{#if settings.report_note}
						<p class="mt-3 text-[13px] {rowTone(settings.report_note.tone)}">
							{settings.report_note.value}
						</p>
					{/if}
				</section>
			{/if}
		</div>
	</div>
{/if}
