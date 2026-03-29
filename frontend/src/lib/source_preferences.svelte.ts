import { browser } from '$app/environment';

import type { Source } from '$lib/client';
import { LocalStorage } from '$lib/storage.svelte';

export const COOKIE_NAME = 'selected_sources';

export const supported_sources: Source[] = ['mta_subway', 'mta_bus', 'njt_bus'];
export const default_sources: Source[] = ['mta_subway', 'mta_bus'];

/**
 * Parses and validates a source string (from cookie or localStorage).
 * Returns a normalized list of valid sources.
 */
export function parse_sources(value: string | null | undefined): Source[] {
	if (!value) return default_sources;

	try {
		const parsed = value
			.split(',')
			.map((s) => s.trim())
			.filter((s): s is Source => supported_sources.includes(s as Source));

		return parsed.length > 0 ? parsed : default_sources;
	} catch {
		return default_sources;
	}
}

/**
 * Serializes sources for storage in a cookie or localStorage.
 */
export function serialize_sources(sources: Source[]): string {
	return sources.join(',');
}

/**
 * Client-side reactive mirror of source preferences.
 * Uses LocalStorage for persistence and syncs to cookies on change.
 */
export class SourcePreferences {
	#storage = new LocalStorage<Source[]>(COOKIE_NAME, default_sources);

	get current() {
		return this.#storage.current;
	}

	set current(value: Source[]) {
		// Enforce at least one source
		const next = value.filter((s) => supported_sources.includes(s));
		const final = next.length > 0 ? next : default_sources;

		this.#storage.current = final;

		if (browser) {
			// Sync to cookie for SSR
			const serialized = serialize_sources(final);
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
export const source_preferences = new SourcePreferences();
