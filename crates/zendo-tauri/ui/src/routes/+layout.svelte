<script lang="ts">
	import './layout.css';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { listen } from '@tauri-apps/api/event';
	import { onMount } from 'svelte';
	import type { LayoutProps } from './$types';
	import GameMode from '#lib/components/game-mode.svelte';
	import TopBar from '#lib/components/top-bar.svelte';
	import { downloads } from '#lib/downloads.svelte.js';
	import { gameMode } from '#lib/game_mode.svelte.js';
	import { library } from '#lib/library.svelte.js';
	import { app } from '#lib/state.svelte.js';

	let { children }: LayoutProps = $props();

	// F11 is Game Mode, which takes the whole screen; the screen keeps the
	// keys while it shows (the arrows, Enter, Escape and Tab); Ctrl+F goes to
	// the Library's search.
	function key(event: KeyboardEvent) {
		if (gameMode.key(event)) {
			return;
		}
		if (event.key === 'F11') {
			event.preventDefault();
			gameMode.toggle();
		} else if (event.key === 'f' && (event.ctrlKey || event.metaKey)) {
			const search = document.getElementById('library-search');
			if (search instanceof HTMLInputElement) {
				event.preventDefault();
				search.focus();
			}
		}
	}

	// The queue's line stays at the foot of the window on every page but
	// Downloads, which shows the running job itself.
	let elsewhere = $derived(page.route.id !== '/downloads');

	onMount(() => {
		void app.start().then(() => gameMode.start());
		void library.start();
		void downloads.start();
		window.addEventListener('keydown', key);
		const asked = listen('downloads_asked', () => void goto(resolve('/downloads')));
		return () => {
			window.removeEventListener('keydown', key);
			void asked.then((stop) => stop());
		};
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
	{#if page.route.id === '/'}
		<GameMode />
	{/if}
	{#if elsewhere && downloads.summary}
		<p class="border-t border-line px-6 py-2 text-sm text-dim">{downloads.summary}</p>
	{/if}
	{#if app.error}
		<p class="border-t border-line px-6 py-2 text-sm text-bad">{app.error}</p>
	{/if}
</div>
