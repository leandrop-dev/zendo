<script lang="ts">
	import { listen } from '@tauri-apps/api/event';
	import { onMount } from 'svelte';
	import {
		accountsCancel,
		accountsCodeEdited,
		accountsFinish,
		accountsGet,
		accountsSetGogGalaxy,
		accountsSetGogPlaytime,
		accountsSignOut,
		accountsStart,
		accountsSteamCancel,
		accountsSteamSignIn,
		accountsSteamSignOut,
		accountsUseBrowser,
		accountsXboxOpen,
		inTauri,
		type AccountsPage,
		type AccountCard
	} from '#lib/api.js';
	import { app } from '#lib/state.svelte.js';

	let accounts = $state<AccountsPage | null>(null);
	let error = $state<string | null>(null);
	// The code typed in the browser flow, kept apart so an answer from another
	// press does not clear it.
	let code = $state('');

	async function load(): Promise<void> {
		if (!inTauri()) {
			return;
		}
		try {
			const page = await accountsGet();
			if (page.code !== code) {
				code = page.code;
			}
			accounts = page;
		} catch (err) {
			error = String(err);
		}
	}

	async function act(action: () => Promise<AccountsPage>): Promise<void> {
		try {
			accounts = await action();
		} catch (err) {
			error = String(err);
		}
	}

	function tone(statusTone: number): string {
		return statusTone === 1 ? 'text-good' : statusTone === 2 ? 'text-bad' : 'text-dim';
	}

	function onCodeInput(event: Event): void {
		code = (event.currentTarget as HTMLInputElement).value;
		void act(() => accountsCodeEdited(code));
	}

	function startOrSignOut(card: AccountCard, index: number): void {
		if (card.action_kind === 0) {
			void act(() => accountsStart(index));
		} else {
			void act(() => accountsSignOut(index));
		}
	}

	onMount(() => {
		void load();
		const changed = listen('accounts_changed', () => void load());
		return () => {
			void changed.then((stop) => stop());
		};
	});
</script>

<svelte:head>
	<title>Accounts · Zendo</title>
</svelte:head>

{#if error}
	<p class="p-8 text-sm text-bad">{error}</p>
{:else if !accounts}
	<p class="p-8 text-sm text-dim">{app.t('library-looking')}</p>
{:else}
	<div class="h-full overflow-y-auto p-6">
		<div class="mx-auto flex max-w-[960px] flex-col gap-4">
			<section class="border border-line bg-surface p-4">
				<p class="text-[13px] text-dim">{app.t('accounts-about')}</p>
			</section>

			<!-- Steam's card. -->
			<section class="border border-line bg-surface p-4">
				<div class="flex items-center gap-3">
					<span
						class="border border-accent-mark px-2 py-0.5 text-[11px] font-bold text-accent-mark uppercase"
					>
						STEAM
					</span>
					<span class="flex-1 text-[16px] font-bold">STEAM</span>
					<button
						type="button"
						onclick={() => {
							if (accounts!.steam.state === 0) {
								void act(accountsSteamSignIn);
							} else if (accounts!.steam.state === 1) {
								void act(accountsSteamCancel);
							} else {
								void act(accountsSteamSignOut);
							}
						}}
						class={accounts.steam.state === 0
							? 'border border-accent-mark bg-accent-mark px-4 py-2 text-[13px] font-bold text-background uppercase'
							: 'border border-line px-4 py-2 text-[13px] uppercase hover:bg-raised'}
					>
						{accounts.steam.state === 0
							? app.t('steam-sign-in')
							: accounts.steam.state === 1
								? app.t('cancel')
								: app.t('accounts-sign-out')}
					</button>
				</div>
				{#if accounts.steam.status}
					<p class="mt-2 text-[13px] {accounts.steam.state === 2 ? 'text-good' : ''}">
						{accounts.steam.status}
					</p>
				{/if}
				<p class="mt-1 text-[13px] text-dim">{accounts.steam.about}</p>
				{#if accounts.steam.family}
					<p class="mt-1 text-[13px]">{accounts.steam.family}</p>
				{/if}
				{#if accounts.steam.note}
					<p class="mt-2 text-[13px] {accounts.steam.note_good ? 'text-good' : 'text-bad'}">
						{accounts.steam.note}
					</p>
				{/if}
				{#if accounts.steam.family_note}
					<p class="mt-1 text-[13px] text-bad">{accounts.steam.family_note}</p>
				{/if}
				{#if accounts.steam.kept}
					<p class="mt-1 text-[13px] {accounts.steam.kept_bad ? 'text-bad' : 'text-dim'}">
						{accounts.steam.kept}
					</p>
				{/if}
			</section>

			<!-- The stores. -->
			{#each accounts.cards as card, index (card.title)}
				<section class="border border-line bg-surface p-4">
					<div class="flex items-center gap-3">
						<span
							class="border border-accent-mark px-2 py-0.5 text-[11px] font-bold text-accent-mark uppercase"
						>
							{card.badge}
						</span>
						<span class="flex-1 text-[16px] font-bold">{card.title}</span>
						<button
							type="button"
							disabled={!card.action_on}
							onclick={() => startOrSignOut(card, index)}
							class={card.action_kind === 0
								? 'border border-accent-mark bg-accent-mark px-4 py-2 text-[13px] font-bold text-background uppercase disabled:opacity-60'
								: 'border border-line px-4 py-2 text-[13px] uppercase hover:bg-raised disabled:opacity-60'}
						>
							{card.action}
						</button>
					</div>
					<p class="mt-2 text-[13px] {tone(card.status_tone)}">{card.status}</p>
					<p
						class="mt-1 text-[13px] {card.engine_ok
							? 'font-mono text-[12px] text-dim'
							: 'text-bad'}"
					>
						{card.engine}
					</p>
					{#if card.gog}
						<label class="mt-3 flex items-center gap-2 text-[13px]">
							<input
								type="checkbox"
								checked={accounts!.gog_playtime}
								onchange={(event) =>
									void act(() => accountsSetGogPlaytime(event.currentTarget.checked))}
							/>
							{app.t('accounts-gog-playtime')}
						</label>
						<label class="mt-1 flex items-center gap-2 text-[13px]">
							<input
								type="checkbox"
								checked={accounts!.gog_galaxy}
								onchange={(event) =>
									void act(() => accountsSetGogGalaxy(event.currentTarget.checked))}
							/>
							{app.t('accounts-gog-galaxy')}
						</label>
					{/if}
					{#if card.flow === 1}
						<div class="mt-3 flex flex-col gap-2">
							{#if card.line}
								<p class="text-[13px]">{card.line}</p>
							{/if}
							{#if card.note}
								<p class="text-[13px] text-bad">{card.note}</p>
							{/if}
							<div class="flex flex-wrap gap-2">
								{#if card.retry}
									<button
										type="button"
										onclick={() => void act(() => accountsStart(index))}
										class="border border-accent-mark bg-accent-mark px-3 py-1 text-[13px] font-bold text-background uppercase"
									>
										{app.t('try-again')}
									</button>
								{/if}
								<button
									type="button"
									disabled={!card.browser_on}
									onclick={() => void act(accountsUseBrowser)}
									class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
								>
									{app.t('accounts-use-browser')}
								</button>
								<button
									type="button"
									onclick={() => void act(accountsCancel)}
									class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
								>
									{app.t('cancel')}
								</button>
							</div>
						</div>
					{:else if card.flow === 2}
						<div class="mt-3 flex flex-col gap-2">
							<div class="flex flex-wrap items-center gap-2">
								<span class="text-[13px]">{card.step_one}</span>
								<button
									type="button"
									disabled={!card.open_on}
									onclick={() => void act(accountsUseBrowser)}
									class="border border-line px-2 py-1 text-[13px] uppercase hover:bg-raised disabled:opacity-60"
								>
									{card.open_label}
								</button>
							</div>
							<p class="text-[13px]">{card.step_two}</p>
							<p class="text-[13px]">{card.step_three}</p>
							<div class="flex flex-wrap gap-2">
								<input
									type="text"
									value={code}
									oninput={onCodeInput}
									placeholder={app.t('accounts-paste-here')}
									class="min-w-0 flex-1 border border-line bg-background px-2 py-1 text-[13px]"
								/>
								<button
									type="button"
									disabled={!card.finish_on}
									onclick={() => void act(accountsFinish)}
									class="border border-accent-mark bg-accent-mark px-3 py-1 text-[13px] font-bold text-background uppercase disabled:opacity-60"
								>
									{card.finish_label}
								</button>
								<button
									type="button"
									onclick={() => void act(accountsCancel)}
									class="border border-line px-3 py-1 text-[13px] uppercase hover:bg-raised"
								>
									{app.t('cancel')}
								</button>
							</div>
							{#if card.note}
								<p class="text-[13px] text-bad">{card.note}</p>
							{/if}
						</div>
					{/if}
				</section>
			{/each}

			<!-- Nexus Launcher. -->
			<section class="border border-line bg-surface p-4">
				<div class="flex items-center gap-3">
					<span
						class="border border-accent-mark px-2 py-0.5 text-[11px] font-bold text-accent-mark uppercase"
					>
						{accounts.xbox.badge}
					</span>
					<span class="flex-1 text-[16px] font-bold">{accounts.xbox.title}</span>
					<button
						type="button"
						disabled={!accounts.xbox.action_on}
						onclick={() => void act(accountsXboxOpen)}
						class="border border-accent-mark bg-accent-mark px-4 py-2 text-[13px] font-bold text-background uppercase disabled:opacity-60"
					>
						{accounts.xbox.action}
					</button>
				</div>
				<p class="mt-2 text-[13px] {accounts.xbox.status_tone === 1 ? 'text-good' : 'text-dim'}">
					{accounts.xbox.status}
				</p>
				<p class="mt-1 text-[13px] text-dim">{accounts.xbox.about}</p>
				{#if accounts.xbox.path}
					<p class="font-mono text-[12px] break-all text-dim">{accounts.xbox.path}</p>
				{/if}
				{#if accounts.xbox.line}
					<p class="text-[13px] text-dim">{accounts.xbox.line}</p>
				{/if}
				{#if accounts.xbox.note}
					<p class="mt-1 text-[13px] {accounts.xbox.note_tone === 1 ? 'text-good' : 'text-bad'}">
						{accounts.xbox.note}
					</p>
				{/if}
			</section>
		</div>
	</div>
{/if}
