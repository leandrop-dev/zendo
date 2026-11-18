import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vitest/config';

export default defineConfig(({ mode }) => ({
	plugins: [
		tailwindcss(),
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			// Tauri has no server: the app is a SPA, served from `ui/build`,
			// with every route falling back to `index.html`.
			adapter: adapter({ fallback: 'index.html' })
		})
	],
	// Tauri's dev loop logs to the same terminal as Vite; keep its lines.
	clearScreen: false,
	server: {
		// `tauri.conf.json`'s `devUrl`; strict, so a busy port fails loudly
		// instead of opening the window on nothing.
		port: 1420,
		strictPort: true
	},
	// Tests mount components in jsdom, so Svelte must resolve its client
	// build rather than the server one it hands the build step.
	resolve: mode === 'test' ? { conditions: ['browser'] } : undefined,
	test: {
		environment: 'jsdom',
		include: ['src/**/*.{test,spec}.{js,ts}'],
		exclude: ['src/lib/server/**']
	}
}));
