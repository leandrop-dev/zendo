<script lang="ts">
	import type { SettingsSetRow } from '#lib/api.js';

	// A list of rows, each with a title, its lines and up to two buttons: the
	// main action (Download, Update, Open) and Remove. An empty text is not
	// drawn.
	let {
		rows,
		onact = () => {},
		onremove = () => {}
	}: {
		rows: SettingsSetRow[];
		onact?: (index: number) => void;
		onremove?: (index: number) => void;
	} = $props();

	function noteTone(tone: number): string {
		return tone === 2 ? 'text-bad' : tone === 1 ? 'text-good' : 'text-dim';
	}
</script>

<div class="flex flex-col gap-3">
	{#each rows as row, index (row.title + row.path)}
		<div class="flex items-start gap-3">
			<div class="min-w-0 flex-1">
				<p class="text-[13px] font-bold">{row.title}</p>
				{#if row.sub}
					<p class="text-[12px] text-dim">{row.sub}</p>
				{/if}
				{#if row.path}
					<p class="font-mono text-[12px] break-all text-dim">{row.path}</p>
				{/if}
				{#if row.note}
					<p class="text-[12px] {noteTone(row.note_tone)}">{row.note}</p>
				{/if}
			</div>
			<div class="flex shrink-0 items-center gap-1">
				{#if row.status}
					<span class="text-[13px] {row.status_tone === 1 ? 'text-good' : 'text-dim'}">
						{row.status}
					</span>
				{/if}
				{#if row.action}
					<button
						type="button"
						disabled={!row.action_on}
						onclick={() => onact(index)}
						class="border border-line px-2 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
					>
						{row.action}
					</button>
				{/if}
				{#if row.remove}
					<button
						type="button"
						disabled={!row.remove_on}
						onclick={() => onremove(index)}
						class="border px-2 py-1 text-[13px] uppercase {row.remove_danger
							? 'border-bad text-bad'
							: 'border-line hover:bg-raised'} disabled:opacity-60"
					>
						{row.remove}
					</button>
				{/if}
			</div>
		</div>
	{/each}
</div>
