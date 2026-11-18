<script lang="ts">
	import { convertFileSrc } from '@tauri-apps/api/core';
	import type { Card } from '#lib/api.js';
	import { inTauri } from '#lib/api.js';

	// One card: the cover with its mark box, badge and progress line, and the
	// title and caption under it. The sizes are the Slint window's, 192 by
	// 370: `outline` rather than `border` keeps them exact.
	let {
		card,
		selected,
		marked,
		onclick
	}: { card: Card; selected: boolean; marked: boolean; onclick: () => void } = $props();

	let cover = $derived(card.cover !== null && inTauri() ? convertFileSrc(card.cover) : null);
	let progress = $derived(card.progress === null ? null : Math.min(1, Math.max(0, card.progress)));
</script>

<button
	type="button"
	aria-current={selected ? 'true' : undefined}
	{onclick}
	class="relative h-[370px] w-[192px] shrink-0 p-1.5 text-left outline-2 hover:bg-surface {marked
		? 'outline-good'
		: selected
			? 'outline-accent-mark'
			: 'outline-transparent'}"
>
	<span class="relative block h-[270px] w-[180px] overflow-hidden rounded-[2px] bg-surface">
		{#if cover}
			<img
				src={cover}
				alt=""
				loading="lazy"
				class="h-full w-full object-cover {card.faded ? 'opacity-35' : ''}"
			/>
		{:else}
			<span class="flex h-full w-full items-center justify-center p-3 text-center text-lg">
				{card.title}
			</span>
		{/if}

		{#if progress !== null}
			<span class="absolute bottom-0 left-0 h-[3px] w-full bg-raised">
				<span class="block h-full bg-accent-mark" style="width: {progress * 100}%"></span>
			</span>
		{/if}

		<span
			class="absolute top-2 left-2 size-5 border {marked
				? 'border-good bg-good'
				: 'border-line bg-surface'}"
		></span>

		{#if card.badge}
			<span
				class="absolute top-2 right-2 border bg-background/80 px-1.5 py-0.5 text-[11px] font-bold {card.badge_running
					? 'border-good/50 text-good'
					: 'border-accent-mark/50 text-accent-mark'}"
			>
				{card.badge}
			</span>
		{/if}
	</span>

	<span class="mt-1 block h-[84px] overflow-hidden">
		<span class="line-clamp-2 text-sm">{card.title}</span>
		<span
			class="mt-0.5 line-clamp-2 text-xs {card.caption_accent ? 'text-accent-mark' : 'text-dim'}"
		>
			{card.caption}
		</span>
	</span>
</button>
