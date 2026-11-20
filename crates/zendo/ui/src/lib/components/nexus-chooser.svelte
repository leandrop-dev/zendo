<script lang="ts">
	import { onMount } from 'svelte';
	import { nexus } from '#lib/nexus.svelte.js';
	import { app } from '#lib/state.svelte.js';

	// Which game an `nxm://` link's mod is for, when several installed games
	// matched it or none did. The games are the library's.

	function key(event: KeyboardEvent): void {
		if (nexus.choosing && event.key === 'Escape') {
			event.preventDefault();
			void nexus.cancel();
		}
	}

	onMount(() => {
		window.addEventListener('keydown', key);
		return () => window.removeEventListener('keydown', key);
	});
</script>

{#if nexus.page?.choosing}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-background/80 p-4">
		<button
			type="button"
			class="absolute inset-0 cursor-default"
			aria-label={app.t('cancel')}
			onclick={() => void nexus.cancel()}
		></button>
		<div
			class="relative flex w-[min(520px,100%)] flex-col gap-4 border border-line bg-background p-6"
		>
			<p class="text-sm">{nexus.page.choose_about}</p>
			<div class="flex max-h-[50vh] flex-wrap gap-2 overflow-y-auto">
				{#each nexus.page.choices as title, at (title)}
					<button
						type="button"
						onclick={() => void nexus.choose(at)}
						class="border border-line px-3 py-1 text-[13px] hover:bg-raised"
					>
						{title}
					</button>
				{/each}
			</div>
			<div class="flex justify-end">
				<button
					type="button"
					onclick={() => void nexus.cancel()}
					class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
				>
					{app.t('cancel')}
				</button>
			</div>
		</div>
	</div>
{/if}
