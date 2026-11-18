<script lang="ts">
	import { app } from '#lib/state.svelte.js';

	// A page the port has not reached yet: its name, and that it is still to
	// come. Slice 1 draws the shell; the pages replace these in the slices
	// that follow (docs/tauri-port-plan.md).
	let { title }: { title: string } = $props();
</script>

<div class="flex min-h-[60vh] flex-col items-center justify-center gap-3 p-8">
	<h1 class="text-2xl font-bold tracking-[0.25em] uppercase">{app.t(title)}</h1>
	<p class="text-sm text-dim">{app.t('page-coming')}</p>
</div>
