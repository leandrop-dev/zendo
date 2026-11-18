// The typed boundary to Rust: each function wraps one command, and the
// types match the structs the commands return.

import { invoke } from '@tauri-apps/api/core';

// A language the window speaks.
export type Language = { tag: string; name: string };

// The window's colours, as CSS colours (the Rust side sends hex).
export type Palette = {
	background: string;
	sidebar: string;
	surface: string;
	raised: string;
	line: string;
	foreground: string;
	dim: string;
	accent: string;
	good: string;
	bad: string;
	/// Text drawn on the accent colour.
	on_accent: string;
	light: boolean;
};

// What the shell starts with.
export type Bootstrap = {
	version: string;
	language: string;
	languages: Language[];
	/// The current language's messages that take no values, by id.
	messages: Record<string, string>;
	palette: Palette;
	fullscreen: boolean;
	/// Whether closing the window hides it to the tray.
	tray: boolean;
};

// True inside the Tauri window. In a plain browser (`vite dev` on its own)
// the IPC is not there, so the pages say so and use Zendo's own colours.
export function inTauri(): boolean {
	return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

// The words, the colours and the window's state, once at start.
export async function bootstrap(): Promise<Bootstrap> {
	return invoke<Bootstrap>('bootstrap');
}

// A message with values: `tr('count-games', { count: 3 })`.
export async function tr(id: string, args: Record<string, string | number> = {}): Promise<string> {
	return invoke<string>('tr', { id, args });
}

// Full screen on or off, and which it is now.
export async function toggleFullscreen(): Promise<boolean> {
	return invoke<boolean>('window_fullscreen_toggle');
}
