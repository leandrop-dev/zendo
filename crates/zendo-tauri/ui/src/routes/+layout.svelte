<script lang="ts">
	import './layout.css';
	import { onMount } from 'svelte';
	import type { LayoutProps } from './$types';
	import TopBar from '#lib/components/top-bar.svelte';
	import { app } from '#lib/state.svelte.js';

	let { children }: LayoutProps = $props();

	// F11 is the window's full screen (Game Mode's screen arrives in slice 6);
	// Escape leaves it.
	function key(event: KeyboardEvent) {
		if (event.key === 'F11') {
			event.preventDefault();
			void app.toggleFullscreen();
		} else if (event.key === 'Escape' && (app.data?.fullscreen ?? false)) {
			void app.toggleFullscreen();
		}
	}

	onMount(() => {
		void app.start();
		window.addEventListener('keydown', key);
		return () => window.removeEventListener('keydown', key);
	});
</script>

<svelte:head>
	<title>Zendo</title>
</svelte:head>

<div class="flex min-h-screen flex-col bg-background text-foreground">
	<TopBar />
	<main class="flex-1">
		{@render children()}
	</main>
	{#if app.error}
		<p class="border-t border-line px-6 py-2 text-sm text-bad">{app.error}</p>
	{/if}
</div>
