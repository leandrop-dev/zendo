<script lang="ts">
	import type { SettingsBar } from '#lib/api.js';
	import { app } from '#lib/state.svelte.js';

	// A download in progress: its name, the bytes so far, Stop and the bar.
	let { bar, onstop = () => {} }: { bar: SettingsBar; onstop?: () => void } = $props();
</script>

{#if bar.active}
	<div class="flex flex-col gap-2">
		<div class="flex items-center gap-3">
			<span class="flex-1 text-[13px]">{bar.name}</span>
			<span class="text-[12px] text-dim">{bar.size}</span>
			<button
				type="button"
				disabled={bar.stopping}
				onclick={onstop}
				class="border border-line px-2 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
			>
				{bar.stopping ? app.t('stopping') : app.t('stop')}
			</button>
		</div>
		<div class="h-2 w-full bg-raised">
			<div class="h-2 bg-accent-mark" style={`width: ${Math.min(bar.percent, 100)}%`}></div>
		</div>
	</div>
{/if}
