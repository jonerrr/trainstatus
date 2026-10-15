import { createContext } from 'svelte';

import { createSubscriber, SvelteURL } from 'svelte/reactivity';

import { goto } from '$app/navigation';
import { page } from '$app/state';

import { is_source } from '#lib/sources.js';

/**
 * Unix seconds from `?at=`. Blank and non-numeric values mean live, so callers
 * use the layout's current-time context instead of parsing the param again.
 */
export function atParamToUnixSeconds(value: string | null | undefined): number | undefined {
	if (value == null || value.trim() === '') return undefined;
	const at = Number(value);
	return Number.isFinite(at) ? at : undefined;
}

/** Re-runs live readers every 15 seconds while any are mounted. */
const live_clock = createSubscriber((update) => {
	const interval = setInterval(update, 15_000);
	return () => clearInterval(interval);
});

export function createCurrentTime(initial?: number) {
	let time = $state<number | undefined>(
		typeof initial === 'number' && Number.isFinite(initial) ? initial : undefined
	);

	return {
		// returns undefined here bc some components need to know if it was user specified
		get value(): number | undefined {
			return time;
		},

		get ms(): number {
			if (time !== undefined) return time * 1000;
			live_clock();
			return Date.now();
		},

		set value(newValue: number | undefined) {
			// js time is in milliseconds
			time = typeof newValue === 'number' && Number.isFinite(newValue) ? newValue : undefined;
		}
	};
}
export const [getCurrentTime, setCurrentTime] =
	createContext<ReturnType<typeof createCurrentTime>>();

export type ModalData = Exclude<App.PageState['modal'], null>;

/** URL search param keys for each modal type */
export const MODAL_PARAM = {
	stop: 's',
	route: 'r',
	trip: 't'
} as const satisfies Record<string, 'r' | 's' | 't'>;

export type ModalParamKey = (typeof MODAL_PARAM)[keyof typeof MODAL_PARAM];
/** Resolve supported links; trips must carry their route context. */
export function modal_from_url(url: {
	searchParams: Pick<URLSearchParams, 'get'>;
}): ModalData | null {
	const params = url.searchParams;
	const source = params.get('src');
	if (!is_source(source)) return null;
	const src = source;
	const trip_id = params.get('t');
	if (trip_id) {
		const route_id = params.get('r');
		return route_id ? { type: 'trip', source: src, route_id, id: trip_id } : null;
	}
	const stop_id = params.get('s');
	if (stop_id) return { type: 'stop', source: src, id: stop_id };
	const route_id = params.get('r');
	if (route_id) return { type: 'route', source: src, id: route_id };
	return null;
}

/**
 * Open a stop/route/trip modal and update the URL to reflect the open state.
 * Pushes a history entry so the back button closes the modal.
 * Preserves existing URL params (e.g. ?at=).
 */
export function open_modal(state: ModalData) {
	const key = MODAL_PARAM[state.type];
	const url = new SvelteURL(page.url.href);

	// Remove any existing modal params to avoid stacking them
	for (const k of Object.values(MODAL_PARAM)) url.searchParams.delete(k);
	url.searchParams.delete('src');

	url.searchParams.set(key, state.id);
	if (state.type === 'trip' && state.route_id) url.searchParams.set('r', state.route_id);
	url.searchParams.set('src', state.source);

	const snapshot = $state.snapshot(state);
	const current_index = page.state?.index ?? 0;

	goto(url.pathname + url.search, {
		shallow: true,
		state: { modal: snapshot, index: current_index + 1 }
	});
}

/**
 * Close the currently open modal and remove its search params.
 * A list-opened modal pushed a history entry, so closing pushes the dismissed
 * entry and Back restores the modal. A fresh load replaces the current entry.
 */
export function close_modal(at: number | undefined, replace = false) {
	const url = new SvelteURL(page.url.href);
	for (const k of Object.values(MODAL_PARAM)) url.searchParams.delete(k);
	url.searchParams.delete('src');
	// Modal callers pass their layout's time to preserve changes before URL sync.
	if (at !== undefined) {
		url.searchParams.set('at', at.toString());
	} else {
		url.searchParams.delete('at');
	}
	const opened_from_list = (page.state?.index ?? 0) > 0;
	goto(url.pathname + url.search, {
		shallow: true,
		replace: replace || !opened_from_list,
		state: { modal: null }
	});
}
