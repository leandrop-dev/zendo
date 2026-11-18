<script lang="ts">
	import type { Chip } from '#lib/api.js';
	import { library } from '#lib/library.svelte.js';

	// A row of chips: the views in the top bar, the sources, the orders and
	// the organize actions on the Library page. Each sends its key to Rust,
	// which knows what it does.
	let { chips, class: klass = '' }: { chips: Chip[]; class?: string } = $props();
</script>

{#if chips.length > 0}
	<nav class="flex flex-wrap gap-2 {klass}">
		{#each chips as chip (chip.key)}
			<button
				type="button"
				aria-pressed={chip.chosen}
				onclick={() => void library.pick(chip.key)}
				class="border px-3 py-1 text-[13px] uppercase {chip.chosen
					? 'border-accent-mark text-accent-mark'
					: 'border-line text-foreground hover:bg-raised'}"
			>
				{chip.label}
				{#if chip.count !== null}
					<span class="ml-1.5 text-xs {chip.chosen ? 'text-accent-mark' : 'text-dim'}">
						{chip.count}
					</span>
				{/if}
			</button>
		{/each}
	</nav>
{/if}
