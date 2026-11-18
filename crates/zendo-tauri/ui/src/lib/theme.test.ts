import { describe, expect, it } from 'vitest';
import { applyPalette, DEFAULT_PALETTE } from './theme.js';

describe('applying a palette', () => {
	it('writes the colours as CSS variables and follows light or dark', () => {
		const style = document.documentElement.style;

		applyPalette({ ...DEFAULT_PALETTE, background: '#010203', accent: '#abcdef', light: true });
		expect(style.getPropertyValue('--background')).toBe('#010203');
		expect(style.getPropertyValue('--primary')).toBe('#abcdef');
		expect(document.documentElement.classList.contains('dark')).toBe(false);
		expect(style.colorScheme).toBe('light');

		// Zendo's own palette is the dark one.
		applyPalette(DEFAULT_PALETTE);
		expect(style.getPropertyValue('--background')).toBe('#171717');
		expect(document.documentElement.classList.contains('dark')).toBe(true);
	});
});
