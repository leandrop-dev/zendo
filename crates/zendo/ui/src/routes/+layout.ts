// Tauri has no server: render every route in the webview, from the SPA
// fallback `ui/build/index.html` (see `vite.config.ts`'s adapter).
export const ssr = false;
export const prerender = false;
