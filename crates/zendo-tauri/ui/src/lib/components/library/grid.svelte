<script lang="ts">
	import { onMount } from 'svelte';
	import {
		CARD_HEIGHT,
		GAP,
		MARGIN,
		columnsFor,
		leftFor,
		rowsFor,
		visibleRows
	} from '#lib/grid.js';
	import { library } from '#lib/library.svelte.js';
	import CardView from './card.svelte';

	// The grid, virtualized by rows: only the rows on screen and two each side
	// are in the DOM, which is what held 60 fps on 2,700 covers in the spike.
	let viewport: HTMLDivElement | undefined = $state();
	let width = $state(0);
	let height = $state(0);
	let scrollTop = $state(0);

	let columns = $derived(columnsFor(width));
	let rows = $derived(rowsFor(library.shown.length, columns));
	let left = $derived(leftFor(width, columns));
	let near = $derived(visibleRows(scrollTop, height, rows));

	// Rust keeps the columns and the viewport for the cursor's paging.
	$effect(() => {
		void library.fit(columns, height);
	});

	// The cursor can leave the drawn rows, so scrolling follows it.
	$effect(() => {
		const cursor = library.cursor;
		if (!viewport || columns < 1) {
			return;
		}
		const top = MARGIN + Math.floor(cursor / columns) * (CARD_HEIGHT + GAP);
		const bottom = top + CARD_HEIGHT;
		const viewTop = viewport.scrollTop;
		const viewBottom = viewTop + viewport.clientHeight;
		if (top < viewTop || bottom > viewBottom) {
			viewport.scrollTop = Math.max(0, top - MARGIN - GAP);
		}
	});

	onMount(() => {
		if (!viewport) {
			return;
		}
		const observer = new ResizeObserver(([entry]) => {
			width = entry.contentRect.width;
			height = entry.contentRect.height;
		});
		observer.observe(viewport);
		return () => observer.disconnect();
	});
</script>

{#if library.empty}
	<div class="flex h-full flex-col items-center justify-center gap-3 p-8 text-center">
		<p class="text-xl tracking-[0.2em] uppercase">{library.empty.title}</p>
		{#if library.empty.about}
			<p class="max-w-lg text-sm text-dim">{library.empty.about}</p>
		{/if}
	</div>
{:else}
	<div
		class="min-h-0 flex-1 overflow-y-auto"
		bind:this={viewport}
		onscroll={() => {
			scrollTop = viewport?.scrollTop ?? 0;
		}}
	>
		<div class="relative" style="height: {MARGIN + rows * (CARD_HEIGHT + GAP)}px">
			{#each Array.from({ length: near.last - near.first }, (_, at) => near.first + at) as row (row)}
				<div
					class="absolute flex"
					style="top: {MARGIN + row * (CARD_HEIGHT + GAP)}px; left: {left}px; gap: {GAP}px"
				>
					{#each library.shown.slice(row * columns, row * columns + columns) as id, at (id)}
						{#if library.cards[id]}
							<CardView
								card={library.cards[id]}
								selected={library.cursor === row * columns + at}
								marked={library.marked.includes(id)}
								onclick={() => void library.click(id)}
							/>
						{/if}
					{/each}
				</div>
			{/each}
		</div>
	</div>
{/if}
