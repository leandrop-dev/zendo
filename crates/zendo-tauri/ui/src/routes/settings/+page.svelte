<script lang="ts">
	import { listen } from '@tauri-apps/api/event';
	import { onMount } from 'svelte';
	import {
		inTauri,
		settingsCopyReport,
		settingsDownloadCovers,
		settingsGet,
		settingsOpen,
		settingsSetAutoUpdate,
		settingsSetCovers,
		settingsSetLanguage,
		settingsSetTheme,
		settingsSetTray,
		settingsSetWorkers,
		type SettingsPage
	} from '#lib/api.js';
	import { app } from '#lib/state.svelte.js';

	// The Settings page's General, Library and Diagnostics groups: one at a
	// time, as the Slint window's are. Keys, Launchers, Engines, Proton and
	// Discord's presence are the next part of this slice.
	const GROUPS = [
		{ key: 'general', label: 'settings-group-general' },
		{ key: 'library', label: 'settings-group-library' },
		{ key: 'diagnostics', label: 'settings-group-diagnostics' }
	] as const;

	let group = $state<(typeof GROUPS)[number]['key']>('general');
	let settings = $state<SettingsPage | null>(null);
	let error = $state<string | null>(null);

	async function load(): Promise<void> {
		if (!inTauri()) {
			return;
		}
		try {
			settings = await settingsGet();
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
