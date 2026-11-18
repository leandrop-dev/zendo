<script lang="ts">
	import { onMount } from 'svelte';
	import Chips from '#lib/components/library/chips.svelte';
	import Grid from '#lib/components/library/grid.svelte';
	import { library } from '#lib/library.svelte.js';

	// The grid's keys: the arrows and the pages move the cursor, and Escape
	// takes back the marks and then the search, as the Slint window does.
	const STEPS: Record<string, string> = {
		ArrowLeft: 'left',
		ArrowRight: 'right',
		ArrowUp: 'up',
		ArrowDown: 'down',
		PageUp: 'page-up',
		PageDown: 'page-down',
		Home: 'first',
		End: 'last'
	};

	function key(event: KeyboardEvent) {
		if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) {
			return;
		}
		const step = STEPS[event.key];
		if (step) {
			event.preventDefault();
			void library.step(step);
		} else if (event.key === 'Escape') {
			event.preventDefault();
			void library.back();
		}
	}

	onMount(() => {
		window.addEventListener('keydown', key);
		return () => window.removeEventListener('keydown', key);
	});
</script>

<div class="flex h-full min-h-0 flex-col">
	<div class="flex flex-wrap items-center gap-2 px-6 pt-6 pb-1">
		<Chips chips={library.chips.sources} />
		<div class="flex-1"></div>
		<Chips chips={library.chips.sorts} />
	</div>
	{#if library.chips.organize.length > 0}
		<div class="px-6 pb-1">
			<Chips chips={library.chips.organize} />
		</div>
	{/if}

	<div class="min-h-0 flex-1">
		<Grid />
	</div>

	{#if library.status}
		<p class="border-t border-line px-6 py-2 text-sm text-dim">{library.status}</p>
	{/if}
</div>
