<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import ArrowDownToLine from '@lucide/svelte/icons/arrow-down-to-line';
	import Settings from '@lucide/svelte/icons/settings';
	import Chips from '#lib/components/library/chips.svelte';
	import { downloads } from '#lib/downloads.svelte.js';
	import { library } from '#lib/library.svelte.js';
	import { app } from '#lib/state.svelte.js';

	// The tabs of the top bar, and the pages the Settings button holds. The
	// route id marks one current: the pathname can differ in the packaged app,
	// whose document is `index.html`.
	const TABS = [
		{ path: '/', route: '/', label: 'page-library' },
		{ path: '/store', route: '/store', label: 'page-store' },
		{ path: '/stats', route: '/stats', label: 'page-stats' }
	] as const;

	const SETTINGS = [
		{ path: '/settings', route: '/settings', label: 'page-settings' },
		{ path: '/accounts', route: '/accounts', label: 'page-accounts' },
		{ path: '/about', route: '/about', label: 'page-about' }
	] as const;

	const tabClass = (current: boolean) =>
		current
			? 'border-accent-mark text-accent-mark'
			: 'border-transparent text-dim hover:bg-raised hover:text-foreground';

	let route = $derived(page.route.id);
	let amongSettings = $derived(SETTINGS.some((item) => item.route === route));
</script>

<header class="bg-sidebar">
	<div class="flex items-center gap-2 border-b border-line px-6 py-3">
		<a href={resolve('/')} class="flex items-center gap-1.5 pr-4">
			<span class="text-xl font-bold">ZENDO</span>
			<span class="text-xl text-accent-mark">•</span>
		</a>

		{#each TABS as tab (tab.path)}
			{@const current = route === tab.route}
			<a
				href={resolve(tab.path)}
				aria-current={current ? 'page' : undefined}
				class="border px-3 py-2 text-[13px] font-bold uppercase {tabClass(current)}"
			>
				{app.t(tab.label)}
				{#if tab.route === '/' && library.ready}
					<span class="ml-1.5 text-xs text-dim">{library.shown.length}</span>
				{/if}
			</a>
		{/each}

		<div class="flex-1"></div>

		{#if route === '/'}
			<Chips chips={library.chips.views} />
			<input
				id="library-search"
				type="search"
				aria-label={app.t('library-search')}
				placeholder={app.t('library-search')}
				value={library.search}
				oninput={(event) => library.setSearch(event.currentTarget.value)}
				class="h-[34px] w-[220px] border border-line bg-surface px-2 text-sm placeholder:text-dim focus:border-accent-mark focus:outline-none"
			/>
		{/if}

		<a
			href={resolve('/downloads')}
			aria-label={app.t('page-downloads')}
			aria-current={route === '/downloads' ? 'page' : undefined}
			class="relative grid size-[34px] place-items-center border {route === '/downloads'
				? 'border-accent-mark text-accent-mark'
				: 'border-transparent text-foreground hover:bg-raised'}"
		>
			<ArrowDownToLine class="size-[18px]" />
			{#if downloads.count > 0}
				<span
					class="absolute top-0 right-0 grid size-[14px] place-items-center bg-accent-mark text-[10px] font-bold text-primary-foreground"
				>
					{downloads.count}
				</span>
			{/if}
		</a>
		<a
			href={resolve('/settings')}
			aria-label={app.t('page-settings')}
			aria-current={amongSettings ? 'page' : undefined}
			class="grid size-[34px] place-items-center border {amongSettings
				? 'border-accent-mark text-accent-mark'
				: 'border-transparent text-foreground hover:bg-raised'}"
		>
			<Settings class="size-[18px]" />
		</a>
	</div>

	{#if amongSettings}
		<nav class="flex gap-2 border-b border-line px-6 py-2">
			{#each SETTINGS as item (item.path)}
				{@const current = route === item.route}
				<a
					href={resolve(item.path)}
					aria-current={current ? 'page' : undefined}
					class="border px-3 py-1 text-[13px] uppercase {current
						? 'border-accent-mark text-accent-mark'
						: 'border-line text-foreground hover:bg-raised'}"
				>
					{app.t(item.label)}
				</a>
			{/each}
		</nav>
	{/if}
</header>
