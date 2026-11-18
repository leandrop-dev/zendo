<script lang="ts">
	import './layout.css';
	import { onMount } from 'svelte';
	import type { LayoutProps } from './$types';
	import TopBar from '#lib/components/top-bar.svelte';
	import { library } from '#lib/library.svelte.js';
	import { app } from '#lib/state.svelte.js';

	let { children }: LayoutProps = $props();

	// F11 is the window's full screen (Game Mode's screen arrives in slice 6);
	// Escape leaves it; Ctrl+F goes to the Library's search.
	function key(event: KeyboardEvent) {
		if (event.key === 'F11') {
			event.preventDefault();
			void app.toggleFullscreen();
		} else if (event.key === 'Escape' && (app.data?.fullscreen ?? false)) {
			void app.toggleFullscreen();
		} else if (event.key === 'f' && (event.ctrlKey || event.metaKey)) {
			const search = document.getElementById('library-search');
			if (search instanceof HTMLInputElement) {
				event.preventDefault();
				search.focus();
			}
		}
	}

	onMount(() => {
		void app.start();
		void library.start();
		window.addEventListener('keydown', key);
		return () => window.removeEventListener('keydown', key);
	});
</script>

<svelte:head>
	<title>Zendo</title>
</svelte:head>

<div class="flex h-screen flex-col overflow-hidden bg-background text-foreground">
	<TopBar />
	<main class="min-h-0 flex-1">
		{@render children()}
	</main>
	{#if app.error}
		<p class="border-t border-line px-6 py-2 text-sm text-bad">{app.error}</p>
	{/if}
</div>
