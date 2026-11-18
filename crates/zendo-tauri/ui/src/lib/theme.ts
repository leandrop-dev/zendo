// The window's colours, from the palette Rust sends: Zendo's own while there
// is none, and whatever the KDE scheme or a theme file says after that.

import type { Palette } from './api.js';

// Zendo's own colours, for a browser preview, where the IPC is not there.
// The same values as `crates/zendo-shell/themes/zendo.toml`; inside the
// Tauri window Rust sends them.
export const DEFAULT_PALETTE: Palette = {
	background: '#171717',
	sidebar: '#121212',
	surface: '#222222',
	raised: '#2c2c2c',
	line: '#303030',
	foreground: '#ccd0cf',
	dim: '#808483',
	accent: '#f25623',
	good: '#7fb069',
	bad: '#e05a4c',
	on_accent: '#101010',
	light: false
};

// The CSS variable each colour fills: shadcn-svelte's own names, and the
// shell's `--zendo-*` ones (see `routes/layout.css`).
const VARIABLES: ReadonlyArray<readonly [string, keyof Palette]> = [
	['--background', 'background'],
	['--foreground', 'foreground'],
	['--card', 'surface'],
	['--card-foreground', 'foreground'],
	['--popover', 'surface'],
	['--popover-foreground', 'foreground'],
	['--primary', 'accent'],
	['--primary-foreground', 'on_accent'],
	['--secondary', 'raised'],
	['--secondary-foreground', 'foreground'],
	['--muted', 'raised'],
	['--muted-foreground', 'dim'],
	['--accent', 'raised'],
	['--accent-foreground', 'foreground'],
	['--destructive', 'bad'],
	['--border', 'line'],
	['--input', 'line'],
	['--ring', 'accent'],
	['--zendo-sidebar', 'sidebar'],
	['--zendo-surface', 'surface'],
	['--zendo-raised', 'raised'],
	['--zendo-line', 'line'],
	['--zendo-dim', 'dim'],
	['--zendo-accent', 'accent'],
	['--zendo-good', 'good'],
	['--zendo-bad', 'bad']
];

// Sets the palette on the document: the variables above, and light or dark
// for the components that switch on it.
export function applyPalette(palette: Palette): void {
	if (typeof document === 'undefined') {
		return;
	}
	const root = document.documentElement;
	for (const [variable, key] of VARIABLES) {
		const value = palette[key];
		if (typeof value === 'string') {
			root.style.setProperty(variable, value);
		}
	}
	root.classList.toggle('dark', !palette.light);
	root.style.colorScheme = palette.light ? 'light' : 'dark';
}
