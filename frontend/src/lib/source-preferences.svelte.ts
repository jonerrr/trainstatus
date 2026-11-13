import { SvelteSet } from 'svelte/reactivity';

import { browser } from '$app/env';

import type { Source } from '#lib/client/index.js';
import { allSources, isSource } from '#lib/sources.js';
import { LocalStorage } from '#lib/storage.svelte.js';

export const COOKIE_NAME = 'selectedSources';

export const defaultSources: Source[] = ['mta_subway', 'mta_bus'];

/**
 * Parses and validates a source string (from cookie or localStorage).
 * Returns a normalized list of valid sources.
 */
export function parseSources(value: string | null | undefined): Source[] {
	if (!value) return defaultSources;

	const selected = value
		.split(',')
		.map((value) => value.trim())
		.filter(isSource);
	return selected.length ? [...new SvelteSet(selected)] : defaultSources;
}
/** Sources a page shows: the saved list plus a deep link's valid `src`. */
export function layoutSources(cookieValue: string | null | undefined, src: string | null) {
	const selected = parseSources(cookieValue);
	return isSource(src) && !selected.includes(src) ? [...selected, src] : selected;
}

/** The saved source list from a `document.cookie` string. */
export function sourcesCookie(cookies: string): string | undefined {
	for (const part of cookies.split(';')) {
		const [name, ...value] = part.trim().split('=');
		if (name === COOKIE_NAME) return decodeURIComponent(value.join('='));
	}
	return undefined;
}

/**
 * Serializes sources for storage in a cookie or localStorage.
 */
export function serializeSources(sources: Source[]): string {
	return sources.join(',');
}

/**
 * Client-side reactive mirror of source preferences.
 * Uses LocalStorage for persistence and syncs to cookies on change.
 */
export class SourcePreferences {
	#storage = new LocalStorage<Source[]>(COOKIE_NAME, defaultSources);

	get current() {
		return this.#storage.current;
	}

	set current(value: Source[]) {
		// Enforce at least one source
		const next = value.filter((s) => allSources.includes(s));
		const final = next.length > 0 ? next : defaultSources;

		this.#storage.current = final;

		if (browser) {
			// Sync to cookie for SSR
			const serialized = serializeSources(final);
			document.cookie = `${COOKIE_NAME}=${serialized}; path=/; max-age=31536000; SameSite=Lax`;
		}
	}

	toggle(source: Source) {
		const current = this.current;
		if (current.includes(source)) {
			this.current = current.filter((s) => s !== source);
		} else {
			this.current = [...current, source];
		}
	}
}
// TODO: will this cause ssr issues?
export const sourcePreferences = new SourcePreferences();
