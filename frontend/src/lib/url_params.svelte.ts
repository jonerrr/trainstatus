import { tick } from 'svelte';

import { pushState, replaceState } from '$app/navigation';
import { page } from '$app/state';

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
			time = newValue;
		}
	};
}

export const current_time = currentTime();

/**
 * Wrap a shallow-routing state change in a View Transition.
 * `direction` is stored on `<html data-modal-direction>` so CSS
 * can pick the right slide animation.
 * Falls back to plain fn() when the API is unavailable (SSR / older browsers).
 */
function with_view_transition(direction: 'forward' | 'backward', fn: () => void) {
	if (typeof document === 'undefined' || !document.startViewTransition) {
		fn();
		return;
	}
	document.documentElement.dataset.modalDirection = direction;
	const transition = document.startViewTransition(async () => {
		fn();
		await tick();
	});
	transition.finished.finally(() => {
		delete document.documentElement.dataset.modalDirection;
	});
}

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
 * Uses pushState so the back button closes the modal.
 * Preserves existing URL params (e.g. ?at=).
 */
export function open_modal(state: ModalData) {
	const key = MODAL_PARAM[state.type];
	const url = new URL(page.url);

	// Remove any existing modal params to avoid stacking them
	for (const k of Object.values(MODAL_PARAM)) url.searchParams.delete(k);
	url.searchParams.set(key, state.id);

	const snapshot = $state.snapshot(state);
	with_view_transition('forward', () => {
		pushState(url.pathname + url.search, { modal: snapshot });
	});
}

/**
 * Close the currently open modal by replacing the current history entry,
 * removing the modal search param from the URL.
 * Works for both push-opened and fresh-load modals.
 */
export function close_modal() {
	const url = new URL(page.url);
	for (const k of Object.values(MODAL_PARAM)) url.searchParams.delete(k);
	// Explicitly sync ?at from the source of truth so we don't lose it due to
	// a race with the layout's $effect.
	// TODO: maybe just manually add ?at to URL instead of using existing url and deleting other params?
	// since there isn't any other possible params right now.
	if (current_time.value !== undefined) {
		url.searchParams.set('at', current_time.value.toString());
	} else {
		url.searchParams.delete('at');
	}
	replaceState(url.pathname + url.search, { modal: null });
}
