//! The typed boundary to Rust. Each function wraps one thing the frontend can
//! do; the commands themselves land slice by slice (docs/tauri-port-plan.md).

import { getVersion } from '@tauri-apps/api/app';

/// True inside the Tauri window. In a plain browser (`vite dev` on its own)
/// the IPC calls are not there, so the pages say so instead of failing.
export function inTauri(): boolean {
	return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/// The app's version, read from the Rust side, or `null` in a browser.
export async function appVersion(): Promise<string | null> {
	return inTauri() ? getVersion() : null;
}
