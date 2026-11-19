<script lang="ts">
	import {
		collectionsClose,
		collectionsCreate,
		collectionsDelete,
		collectionsDraft,
		collectionsRename,
		collectionsRenameDone,
		collectionsRenameDraft,
		collectionsSaveView,
		collectionsSetMember,
		type CollectionsPage
	} from '#lib/api.js';
	import { app } from '#lib/state.svelte.js';

	// The Collections sheet: the hand-picked collections and the saved views
	// in one list, a field to make one, and a tick per collection for the
	// games the sheet was opened for.
	let {
		collections,
		act,
		onapply
	}: {
		collections: CollectionsPage;
		act: (action: () => Promise<CollectionsPage>) => void;
		onapply: (name: string) => void;
	} = $props();

	function toggleRename(row: { renaming: boolean; name: string }): void {
		if (row.renaming) {
			void act(collectionsRenameDone);
		} else {
			void act(() => collectionsRename(row.name));
		}
	}
</script>

<div
	class="absolute inset-0 z-10 grid place-items-center bg-black/60"
	onclick={() => void act(collectionsClose)}
	role="presentation"
>
	<div
		class="flex max-h-[85%] w-[min(620px,calc(100%-3rem))] flex-col gap-4 overflow-y-auto border border-line bg-background p-6"
		onclick={(event) => event.stopPropagation()}
		role="presentation"
	>
		<h2 class="text-[18px] font-bold uppercase">{app.t('collections-title')}</h2>
		<p class="text-[13px] text-dim">{collections.count}</p>

		<div class="flex flex-col gap-2">
			{#if collections.empty}
				<p class="text-[13px] text-dim">{collections.empty}</p>
			{/if}
			{#each collections.rows as row (row.name)}
				<div class="flex items-center gap-2 border-b border-line pb-2">
					{#if row.checkable}
						<input
							type="checkbox"
							checked={row.checked}
							onchange={(event) =>
								void act(() => collectionsSetMember(row.name, event.currentTarget.checked))}
						/>
					{/if}
					<div class="min-w-0 flex-1">
						{#if row.renaming}
							<input
								type="text"
								value={collections.rename_draft}
								oninput={(event) =>
									void act(() => collectionsRenameDraft(event.currentTarget.value))}
								onkeydown={(event) => {
									if (event.key === 'Enter') {
										void act(collectionsRenameDone);
									}
								}}
								placeholder={row.name}
								class="w-full border border-line bg-surface px-2 py-1 text-[13px]"
							/>
						{:else}
							<p class="text-[15px]">{row.name}</p>
							{#if row.sub}
								<p class="text-[11px] text-dim">{row.sub}</p>
							{/if}
						{/if}
					</div>
					{#if !row.view && !row.renaming}
						<span class="w-[90px] text-right text-[12px] text-dim">{row.count}</span>
					{/if}
					{#if row.view && !row.renaming}
						<button
							type="button"
							onclick={() => onapply(row.name)}
							class="border border-line px-2 py-1 text-[13px] uppercase hover:bg-raised"
						>
							{app.t('collections-apply')}
						</button>
					{/if}
					<button
						type="button"
						onclick={() => toggleRename(row)}
						class="border border-line px-2 py-1 text-[13px] uppercase hover:bg-raised"
					>
						{row.renaming ? app.t('collections-ok') : app.t('collections-rename')}
					</button>
					<button
						type="button"
						onclick={() => void act(() => collectionsDelete(row.name))}
						class="border px-2 py-1 text-[13px] uppercase {row.confirm
							? 'border-bad text-bad'
							: 'border-line hover:bg-raised'}"
					>
						{row.confirm ? app.t('collections-really-delete') : app.t('collections-delete')}
					</button>
				</div>
			{/each}
		</div>

		<p class="text-[12px] text-dim">{collections.now}</p>

		<div class="flex flex-wrap gap-2">
			<input
				type="text"
				value={collections.draft}
				oninput={(event) => void act(() => collectionsDraft(event.currentTarget.value))}
				onkeydown={(event) => {
					if (event.key === 'Enter') {
						void act(collectionsCreate);
					}
				}}
				placeholder={app.t('collections-new-placeholder')}
				class="min-w-0 flex-1 border border-line bg-surface px-2 py-1 text-[13px]"
			/>
			<button
				type="button"
				onclick={() => void act(collectionsCreate)}
				class="border border-accent-mark bg-accent-mark px-3 py-1 text-[13px] font-bold text-background uppercase"
			>
				{app.t('collections-new')}
			</button>
			<button
				type="button"
				onclick={() => void act(collectionsSaveView)}
				class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
			>
				{app.t('collections-save-view')}
			</button>
		</div>

		{#if collections.note}
			<p class="text-[13px] {collections.note.tone === 1 ? 'text-good' : 'text-bad'}">
				{collections.note.value}
			</p>
		{/if}

		<div class="flex justify-end">
			<button
				type="button"
				onclick={() => void act(collectionsClose)}
				class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
			>
				{app.t('collections-cancel')}
			</button>
		</div>
	</div>
</div>
