<script lang="ts">
	import { onMount } from 'svelte';
	import type { OptionsChip, OptionsSwitch } from '#lib/api.js';
	import { options, type OptionsFields } from '#lib/options.svelte.js';
	import { app } from '#lib/state.svelte.js';

	// The Launch options sheet: a card over the game's page with a scrolling
	// form and Cancel and Save at its foot. Rust keeps the form
	// (`zendo_shell::launch_options`); nothing here decides an option.

	let sheet = $derived(options.sheet);
	let fields = $derived(options.fields);

	// Enter in a field saves; Escape closes (asking first with changes). The
	// page's own keys stay quiet while the sheet is open.
	function key(event: KeyboardEvent): void {
		if (!options.open) {
			return;
		}
		if (event.key === 'Escape') {
			event.preventDefault();
			options.close();
		} else if (event.key === 'Enter' && event.target instanceof HTMLInputElement) {
			event.preventDefault();
			void options.save();
		}
	}

	onMount(() => {
		window.addEventListener('keydown', key);
		return () => window.removeEventListener('keydown', key);
	});
</script>

{#snippet chips(list: number, rows: OptionsChip[])}
	<div class="flex flex-wrap gap-2">
		{#each rows as chip (chip.id)}
			<button
				type="button"
				aria-pressed={chip.chosen}
				onclick={() => options.choose(list, chip.id)}
				class="border px-3 py-1 text-[13px] {chip.chosen
					? 'border-accent-mark text-accent-mark'
					: 'border-line text-foreground hover:bg-raised'}"
			>
				{chip.label}
			</button>
		{/each}
	</div>
{/snippet}

{#snippet switches(rows: OptionsSwitch[])}
	<div class="flex flex-col gap-3">
		{#each rows as row (row.id)}
			<div class="flex flex-col gap-1">
				<button
					type="button"
					role="switch"
					aria-checked={row.on}
					disabled={!row.enabled}
					onclick={() => options.toggle(row.id, !row.on)}
					class="flex items-center gap-2 self-start text-left text-sm disabled:cursor-default"
				>
					<span
						class="flex h-4 w-4 shrink-0 items-center justify-center border text-[11px] {row.on
							? 'border-accent-mark bg-accent-mark text-background'
							: 'border-line'} {row.enabled ? '' : 'opacity-55'}"
					>
						{row.on ? '✓' : ''}
					</span>
					<span class={row.enabled ? '' : 'text-dim'}>{row.label}</span>
				</button>
				<p class="pl-6 text-[13px] text-dim">{row.about}</p>
			</div>
		{/each}
	</div>
{/snippet}

{#snippet input(key: keyof OptionsFields, id: number, placeholder: string, width: string)}
	<input
		type="text"
		value={fields[key]}
		{placeholder}
		oninput={(event) => void options.edit(id, event.currentTarget.value)}
		class="border border-line bg-surface px-3 py-1.5 text-sm {width}"
	/>
{/snippet}

{#snippet sizeRow(
	label: string,
	firstId: number,
	secondId: number,
	firstKey: keyof OptionsFields,
	secondKey: keyof OptionsFields
)}
	<div class="flex items-center gap-4">
		<p class="w-[110px] shrink-0 text-sm text-dim">{label}</p>
		{@render input(firstKey, firstId, app.t('options-width'), 'w-[90px]')}
		<span class="text-sm text-dim">×</span>
		{@render input(secondKey, secondId, app.t('options-height'), 'w-[90px]')}
	</div>
{/snippet}

{#snippet heading(label: string, about: string)}
	<div class="flex flex-col gap-1">
		<p class="text-[12px] text-dim uppercase">{label}</p>
		{#if about}
			<p class="text-[13px] text-dim">{about}</p>
		{/if}
	</div>
{/snippet}

{#if sheet?.open}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-background/80 p-4">
		<button
			type="button"
			class="absolute inset-0 cursor-default"
			aria-label={app.t('close')}
			onclick={() => options.close()}
		></button>
		<div
			class="relative flex max-h-[calc(100vh-2rem)] w-[min(660px,100%)] flex-col gap-4 overflow-hidden border border-line bg-background p-6"
		>
			<h2 class="shrink-0 truncate text-lg font-bold uppercase">{sheet.heading}</h2>

			<div class="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto pr-2">
				{#if sheet.windows}
					<div class="flex flex-col gap-2">
						{@render heading('Proton', app.t('options-proton-about'))}
						{@render chips(0, sheet.protons)}
					</div>
				{/if}

				{#if sheet.store !== 0}
					<div class="flex flex-col gap-2">
						{@render heading(app.t('options-task'), app.t('options-task-about'))}
						{#if sheet.has_tasks}
							{@render chips(1, sheet.tasks)}
						{:else}
							<p class="text-sm text-dim">{app.t('options-task-none')}</p>
						{/if}
					</div>
					<div class="flex flex-col gap-2">
						{@render heading(app.t('options-executable'), app.t('options-executable-about'))}
						<div class="flex gap-4">
							<div class="min-w-0 flex-1">
								{@render input('executable', 0, app.t('options-executable-empty'), 'w-full')}
							</div>
							<button
								type="button"
								disabled={sheet.picking}
								onclick={() => options.browse()}
								class="border border-line px-3 py-1.5 text-[13px] uppercase hover:bg-raised disabled:opacity-50"
							>
								{sheet.picking ? app.t('choosing') : app.t('options-executable-browse')}
							</button>
						</div>
					</div>
				{/if}

				{#if sheet.store === 2}
					<div class="flex flex-col gap-2">
						{@render heading(app.t('options-language'), app.t('options-language-about'))}
						{@render input('language', 1, app.t('options-language-empty'), 'w-[200px]')}
					</div>
				{/if}

				{@render switches(sheet.start_switches)}

				{#if sheet.store !== 0 && sheet.windows}
					<div class="flex flex-col gap-3">
						{@render switches(sheet.saves_switch)}
						<div class="flex flex-wrap gap-3">
							{#each sheet.sync_labels as label, at (label)}
								<button
									type="button"
									disabled={sheet.syncing}
									onclick={() => options.sync(at)}
									class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-50"
								>
									{label}
								</button>
							{/each}
						</div>
						{#if sheet.syncing}
							<p class="text-[13px] text-dim">{app.t('saves-syncing')}</p>
						{:else if sheet.saves_note}
							<p
								class="border px-3 py-1.5 text-[13px] {sheet.saves_note_good
									? 'border-good text-good'
									: 'border-bad text-bad'}"
							>
								{sheet.saves_note}
							</p>
						{/if}
					</div>
				{/if}

				{@render switches(sheet.tool_switches)}

				{#if sheet.gamescope_on}
					<div class="flex flex-col gap-4">
						<div class="flex flex-col gap-2">
							<p class="text-sm text-dim">{app.t('options-gamescope-shows')}</p>
							{@render chips(4, sheet.windows_kinds)}
						</div>
						<div class="flex flex-col gap-2">
							{@render sizeRow(app.t('options-output-size'), 2, 3, 'width', 'height')}
							{@render sizeRow(app.t('options-game-size'), 5, 6, 'game_width', 'game_height')}
							<p class="text-[13px] text-dim">{app.t('options-sizes-about')}</p>
							{#if sheet.gamescope_error}
								<p class="border border-bad px-3 py-1.5 text-[13px] text-bad">
									{sheet.gamescope_error}
								</p>
							{/if}
						</div>
						<div class="flex flex-col gap-2">
							<p class="text-sm text-dim">{app.t('options-scaler')}</p>
							{@render chips(5, sheet.scalers)}
						</div>
						<div class="flex flex-col gap-2">
							<p class="text-sm text-dim">{app.t('options-filter')}</p>
							{@render chips(6, sheet.filters)}
						</div>
						{#if sheet.sharpens}
							<div class="flex flex-col gap-2">
								<p class="text-sm text-dim">{app.t('options-fsr-sharpening')}</p>
								{@render chips(7, sheet.sharpness)}
							</div>
						{/if}
						<div class="flex flex-col gap-2">
							<p class="text-sm text-dim">{app.t('options-gamescope-fps')}</p>
							{@render input('fps', 7, app.t('options-gamescope-fps-empty'), 'w-[200px]')}
						</div>
						<div class="flex flex-col gap-2">
							{@render heading(
								app.t('options-gamescope-more'),
								app.t('options-gamescope-more-about')
							)}
							{@render input('more', 8, '--adaptive-sync', 'w-full')}
						</div>
					</div>
				{/if}

				{#if sheet.native}
					<div class="flex flex-col gap-2">
						{@render heading(app.t('options-runtime'), app.t('options-runtime-about'))}
						{@render chips(2, sheet.runtimes)}
					</div>
				{/if}

				{@render switches(sheet.offload_switch)}

				{#if sheet.windows}
					<div class="flex flex-col gap-4">
						<p class="text-[12px] text-dim uppercase">{app.t('options-proton-switches')}</p>
						{@render switches(sheet.proton_a)}
						{#if sheet.fsr_on}
							<div class="flex flex-col gap-2">
								<p class="text-sm text-dim">{app.t('options-fsr-sharpening')}</p>
								{@render chips(3, sheet.fsr_sharpening)}
							</div>
						{/if}
						{@render switches(sheet.proton_b)}
						<div class="flex flex-col gap-2">
							{@render heading(app.t('options-frame-cap'), app.t('options-frame-cap-about'))}
							{@render input('frame_cap', 4, app.t('options-frame-cap-empty'), 'w-[160px]')}
						</div>
						{@render switches(sheet.proton_c)}
					</div>
				{/if}

				<div class="flex flex-col gap-1">
					{@render heading(app.t('local-arguments'), app.t('options-arguments-about'))}
					{@render input('arguments', 9, '-windowed', 'w-full')}
					{#if sheet.arguments_error}
						<p class="border border-bad px-3 py-1.5 text-[13px] text-bad">
							{sheet.arguments_error}
						</p>
					{/if}
				</div>

				<div class="flex flex-col gap-1">
					{@render heading(app.t('options-environment'), app.t('options-environment-about'))}
					{@render input('environment', 10, 'DXVK_HUD=fps', 'w-full')}
					{#if sheet.environment_error}
						<p class="border border-bad px-3 py-1.5 text-[13px] text-bad">
							{sheet.environment_error}
						</p>
					{/if}
				</div>

				<div class="flex flex-col gap-1">
					{@render heading(app.t('options-before'), app.t('options-before-about'))}
					{@render input('before', 11, "notify-send 'Starting'", 'w-full')}
				</div>

				<div class="flex flex-col gap-1">
					{@render heading(app.t('options-after'), app.t('options-after-about'))}
					{@render input('after', 12, '~/bin/back-up-saves', 'w-full')}
				</div>
			</div>

			<div class="flex shrink-0 flex-wrap items-center gap-3 border-t border-line pt-4">
				{#if sheet.confirm_close}
					<p class="min-w-0 flex-1 text-[13px] text-bad">{app.t('options-discard-ask')}</p>
					<button
						type="button"
						onclick={() => options.keepEditing()}
						class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
					>
						{app.t('options-keep-editing')}
					</button>
					<button
						type="button"
						onclick={() => options.discard()}
						class="border border-bad px-3 py-1 text-[13px] text-bad uppercase hover:bg-raised"
					>
						{app.t('options-discard')}
					</button>
				{:else}
					<p class="min-w-0 flex-1 text-right text-[13px] text-bad">{sheet.error}</p>
					<button
						type="button"
						onclick={() => options.close()}
						class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
					>
						{app.t('cancel')}
					</button>
					<button
						type="button"
						onclick={() => void options.save()}
						class="border border-accent-mark bg-accent-mark px-3 py-1 text-[13px] font-bold text-background uppercase"
					>
						{app.t('save')}
					</button>
				{/if}
			</div>
		</div>
	</div>
{/if}
