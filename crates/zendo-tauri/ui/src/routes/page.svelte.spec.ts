import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import Page from './+page.svelte';

describe('the placeholder page', () => {
	it('names the app and says when the IPC is not there', async () => {
		render(Page);

		expect(screen.getByRole('heading', { name: 'Zendo' })).toBeTruthy();
		// jsdom has no Tauri internals, so the page takes its browser path.
		expect((await screen.findByText(/browser preview/)).textContent).toContain('browser preview');
	});
});
