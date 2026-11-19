<script lang="ts">
	import { settingsOpen } from '#lib/api.js';
	import { app } from '#lib/state.svelte.js';

	// The About page: what Zendo is, its licence and what it is built on. It
	// is static; Rust gives the version and where the source is.
	const LICENCE = ['about-licence-text-slint'] as const;
	const BUILT_ON = [
		'about-built-slint',
		'about-built-gilrs',
		'about-built-keyvalues',
		'about-built-ureq',
		'about-built-steamgriddb',
		'about-built-fluent',
		'about-built-font'
	] as const;

	async function open(url: string): Promise<void> {
		await settingsOpen(url);
	}
</script>

<svelte:head>
	<title>About · Zendo</title>
</svelte:head>

<div class="h-full overflow-y-auto p-6">
	<div class="mx-auto flex max-w-[860px] flex-col gap-4">
		<section class="border border-line bg-surface p-4">
			<div class="flex flex-col gap-3">
				<div>
					<p class="text-[40px] font-bold">Zendo</p>
					<p class="text-[13px] text-dim">{app.data?.version_line ?? ''}</p>
				</div>
				<p class="text-[15px]">{app.t('about-intro')}</p>
				<div class="flex flex-wrap gap-2">
					<button
						type="button"
						onclick={() => void open(app.data?.repository ?? '')}
						class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
					>
						{app.t('about-source-code')}
					</button>
					<button
						type="button"
						onclick={() => void open(`${app.data?.repository ?? ''}/issues`)}
						class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
					>
						{app.t('about-report-problem')}
					</button>
				</div>
			</div>
		</section>

		<section class="border border-line bg-surface p-4">
			<h2 class="mb-3 text-[14px] font-bold uppercase">{app.t('about-licence')}</h2>
			<div class="flex flex-col gap-2">
				{#each LICENCE as line (line)}
					<p class="text-[14px]">{app.t(line)}</p>
				{/each}
			</div>
		</section>

		<section class="border border-line bg-surface p-4">
			<h2 class="mb-3 text-[14px] font-bold uppercase">{app.t('about-built-on')}</h2>
			<div class="flex flex-col gap-2">
				{#each BUILT_ON as line (line)}
					<p class="text-[14px]">{app.t(line)}</p>
				{/each}
			</div>
		</section>
	</div>
</div>
