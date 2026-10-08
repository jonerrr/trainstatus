import { SvelteURL } from 'svelte/reactivity';

import { goto } from '$app/navigation';
import { page } from '$app/state';

/**
 * Unix seconds from `?at=`. Blank and non-numeric values mean live, so callers
 * keep using {@link current_time} instead of parsing the param again.
 */
export function atParamToUnixSeconds(value: string | null | undefined): number | undefined {
	if (value == null || value.trim() === '') return undefined;
	const at = Number(value);
	return Number.isFinite(at) ? at : undefined;
}

// if user specified unix timestamp, it is stored here.
function currentTime() {
	let time = $state<number | undefined>();

	return {
		// returns undefined here bc some components need to know if it was user specified
		get value(): number | undefined {
			return time;
		},

		get ms(): number {
			return time ? time * 1000 : new Date().getTime();
		},

		set value(newValue: number | undefined) {
			// js time is in milliseconds
			time = typeof newValue === 'number' && Number.isFinite(newValue) ? newValue : undefined;
		}
	};
}
// TODO: ensure this doesn't cause ssr issues
export const current_time = currentTime();

export type ModalData = Exclude<App.PageState['modal'], null>;

/** URL search param keys for each modal type */
export const MODAL_PARAM = {
	stop: 's',
	route: 'r',
	trip: 't'
} as const satisfies Record<string, 'r' | 's' | 't'>;

export type ModalParamKey = (typeof MODAL_PARAM)[keyof typeof MODAL_PARAM];

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
	// if ('data' in state && 'source' in (state.data as any) && (state.data as any).source) {
	url.searchParams.set('src', state.data.source);
	// }

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
export function close_modal() {
	const url = new SvelteURL(page.url.href);
	for (const k of Object.values(MODAL_PARAM)) url.searchParams.delete(k);
	url.searchParams.delete('src');
	// Explicitly sync ?at from the source of truth so we don't lose it due to
	// a race with the layout's $effect.
	// TODO: maybe just manually add ?at to URL instead of using existing url and deleting other params?
	// since there isn't any other possible params right now.
	if (current_time.value !== undefined) {
		url.searchParams.set('at', current_time.value.toString());
	} else {
		url.searchParams.delete('at');
	}
	const opened_from_list = (page.state?.index ?? 0) > 0;
	goto(url.pathname + url.search, {
		shallow: true,
		replace: !opened_from_list,
		state: { modal: null }
	});
}
