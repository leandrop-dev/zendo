<script lang="ts">
	import { convertFileSrc } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import { inTauri, type InstallerOption } from '#lib/api.js';
	import { installer } from '#lib/installer.svelte.js';
	import { app } from '#lib/state.svelte.js';

	// A mod's FOMOD installer: its steps one at a time, each group's options
	// as radios or boxes, the option last touched described with its picture,
	// then Install. Rust keeps the sheet (`zendo_shell::installer`); nothing
	// here decides an option.
	//
	// The sheet takes every key while it is open, and only Cancel and Escape
	// close it: closing drops the unpacked mod, so a click outside does not.

	let sheet = $derived(installer.sheet);
	let picture = $derived(sheet?.picture && inTauri() ? convertFileSrc(sheet.picture) : null);

	// The arrows move the cursor and the step, Enter picks, Escape closes.
	function key(event: KeyboardEvent): void {
		if (!installer.open) {
			return;
		}
		switch (event.key) {
			case 'ArrowUp':
				event.preventDefault();
				installer.focus(-1);
				break;
			case 'ArrowDown':
				event.preventDefault();
				installer.focus(1);
				break;
			case 'ArrowLeft':
				event.preventDefault();
				installer.back();
				break;
			case 'ArrowRight':
				event.preventDefault();
				installer.next();
				break;
			case 'Enter':
				event.preventDefault();
				installer.activate();
				break;
			case 'Escape':
				event.preventDefault();
				installer.cancel();
				break;
			case 'Tab':
				// The page behind keeps its focus, as the old window kept it.
				event.preventDefault();
				break;
			default:
				break;
		}
	}

	onMount(() => {
		window.addEventListener('keydown', key);
		return () => window.removeEventListener('keydown', key);
	});

	function mark(option: InstallerOption): string {
		const filled = option.on ? 'border-accent-mark bg-accent-mark text-background' : 'border-line';
		return option.single
			? `flex h-4 w-4 shrink-0 items-center justify-center rounded-full border ${filled}`
			: `flex h-4 w-4 shrink-0 items-center justify-center border text-[11px] ${filled}`;
	}
</script>

{#if sheet?.open}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-background/80 p-4">
		<!-- Nothing behind the sheet hears a click, and none closes it. -->
		<div class="absolute inset-0 cursor-default" aria-hidden="true"></div>
		<div
			role="dialog"
			aria-modal="true"
			aria-label={sheet.title}
			class="relative flex h-[calc(100vh-2rem)] w-[min(920px,100%)] flex-col gap-4 overflow-hidden border border-line bg-background p-6"
		>
			<div class="flex shrink-0 flex-col gap-1">
				<h2 class="truncate text-lg font-bold uppercase">{sheet.title}</h2>
				<p class="truncate text-[13px] text-dim">{sheet.step_line}</p>
				{#if sheet.needs}
					<p class="text-[13px] text-bad">{app.t('installer-needs')}</p>
				{/if}
			</div>

			<div class="flex min-h-0 flex-1 gap-6">
				<div class="flex min-w-0 flex-[3] flex-col gap-0.5 overflow-y-auto pr-2">
					{#each sheet.groups as group, at (at)}
						<p class="pt-2 pb-1 text-[14px] font-bold first:pt-0">{group.title}</p>
						{#each group.options as option (option.option)}
							<!-- A list mixes radio groups and boxes: a checkbox that
							reports its checked state is the honest common role. -->
							<button
								type="button"
								role="checkbox"
								aria-checked={option.on}
								disabled={!option.enabled}
								onclick={() => installer.pick(option)}
								class="flex min-h-[26px] items-center gap-2 border px-2 py-0.5 text-left text-[14px] {option.focused
									? 'border-accent-mark bg-raised'
									: 'border-transparent hover:bg-raised'} disabled:cursor-default disabled:hover:bg-transparent"
							>
								<span class={mark(option)}>{option.on && !option.single ? '✓' : ''}</span>
								<span
									class="min-w-0 flex-1 truncate {option.dim || !option.enabled
										? 'text-dim'
										: 'text-foreground'}"
								>
									{option.label}
								</span>
							</button>
						{/each}
					{/each}
				</div>

				<div class="flex min-w-0 flex-[2] flex-col gap-3 overflow-y-auto pr-2">
					{#if picture}
						<div class="h-[220px] w-full shrink-0">
							<img src={picture} alt="" class="h-full w-full object-contain" />
						</div>
					{/if}
					<p class="text-[15px] font-bold">{sheet.about_name}</p>
					<p class="text-[13px] text-dim">{sheet.about_text}</p>
				</div>
			</div>

			<div class="flex shrink-0 items-center gap-3">
				<button
					type="button"
					onclick={() => installer.cancel()}
					class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
				>
					{app.t('cancel')}
				</button>
				<div class="flex-1"></div>
				<button
					type="button"
					disabled={!sheet.back_enabled}
					onclick={() => installer.back()}
					class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-40"
				>
					{app.t('installer-back')}
				</button>
				<button
					type="button"
					onclick={() => installer.next()}
					class="border border-accent-mark bg-accent-mark px-3 py-1 text-[13px] text-background uppercase hover:opacity-90"
				>
					{sheet.primary_label}
				</button>
			</div>
		</div>
	</div>
{/if}
