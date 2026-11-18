<script lang="ts">
	import { onMount } from 'svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { appVersion, inTauri } from '#lib/tauri.js';

	// Slice 0's placeholder: the window, its title and the dev loop. The
	// version comes over IPC, which proves the frontend, the capabilities and
	// the Rust side are wired together; the pages themselves start at slice 1.
	let version = $state<string | null>(null);
	let shell = $state<'tauri' | 'browser'>('browser');

	async function readVersion() {
		shell = inTauri() ? 'tauri' : 'browser';
		version = await appVersion();
	}

	onMount(readVersion);
</script>

<svelte:head>
	<title>Zendo</title>
</svelte:head>

<main
	class="flex min-h-screen flex-col items-center justify-center gap-6 bg-background p-8 text-foreground"
>
	<div class="flex flex-col items-center gap-2">
		<h1 class="text-3xl font-semibold tracking-[0.25em] uppercase">Zendo</h1>
		<p class="max-w-md text-center text-sm text-muted-foreground">
			The Tauri shell is scaffolded. The pages arrive slice by slice.
		</p>
	</div>
	<div class="flex items-center gap-3 font-mono text-xs text-muted-foreground">
		<span data-testid="shell">
			{shell === 'tauri' ? `zendo ${version ?? '…'}` : 'browser preview, no IPC'}
		</span>
		<Button variant="outline" size="xs" onclick={readVersion}>Read again</Button>
	</div>
</main>
