<script lang="ts">
	import { flushSync, untrack } from 'svelte';

	import type { Attachment } from 'svelte/attachments';
	import { on } from 'svelte/events';

	import { page } from '$app/state';

	import type { Source } from '#lib/client/index.js';
	import { contain_modal_focus } from '#lib/modal_focus.js';
	import Pin from '#lib/Pin.svelte';
	import { route_pins, stop_pins, trip_pin_key, trip_pins, type Pins } from '#lib/pins.svelte.js';
	import RouteModal from '#lib/Route/Modal.svelte';
	import StopModal from '#lib/Stop/Modal.svelte';
	import { LocalStorage } from '#lib/storage.svelte.js';
	import { getTransitData, UnknownRouteError } from '#lib/transit/store.svelte.js';
	import TripModal from '#lib/Trip/Modal.svelte';
	import UpdateIndicator from '#lib/UpdateIndicator.svelte';
	import { close_modal, getCurrentTime } from '#lib/url_params.svelte.js';

	import {
		AlarmClock,
		CircleX,
		ClipboardCheck,
		RotateCcwClock,
		Share,
		Timer
	} from '@lucide/svelte';

	const current_time = getCurrentTime();
	const transit = getTransitData();

	function catalog_of(source: Source) {
		return transit.peekCatalog(source);
	}

	// by reassigning the page.state locally, we can ensure the dialog transitions run before the DOM updates.
	// Otherwise, the sliding animation looks like it runs twice.
	let current_page_state = $state.raw(page.state);
	const selected_modal = $derived(current_page_state.modal);
	const has_modal = $derived(!!current_page_state.modal);

	// Raw state preserves modal identity across shallow navigation and animation snapshots.

	let modal_transition: ViewTransition | undefined;

	function slide_to(forward: boolean) {
		document.documentElement.dataset.modalDirection = forward ? 'forward' : 'backward';

		const apply = () => {
			// Catalog validation or another navigation can change the selection while
			// the transition waits to capture its old view. Apply the latest selection.
			current_page_state = page.state;
		};

		// experimental.async makes tick() wait for a frame, which a view transition
		// defers until this callback finishes. Flush on a microtask so we aren't
		// calling flushSync from inside the effect.
		if (document.startViewTransition) {
			const transition = document.startViewTransition(
				() =>
					new Promise<void>((resolve) => {
						queueMicrotask(() => {
							flushSync(apply);
							resolve();
						});
					})
			);
			modal_transition = transition;
			transition.ready.catch(() => {});
			const clear_transition = () => {
				if (modal_transition === transition) modal_transition = undefined;
			};
			transition.finished.then(clear_transition, clear_transition);
		} else {
			apply();
		}
	}

	$effect(() => {
		// $inspect.trace('modal state transition effect');
		const next_state = page.state;
		if (untrack(() => next_state.modal !== current_page_state.modal)) {
			const forward = untrack(() => (next_state.index ?? 0) > (current_page_state?.index ?? 0));
			slide_to(forward);
		}
	});

	const selected_catalog = transit.catalog(() => page.state.modal?.source ?? null);
	const selected_service = transit.route(() => {
		const selected = page.state.modal;
		const catalog = selected_catalog.current.value;
		const route_id = selected?.type === 'trip' ? selected.route_id : null;
		return selected && route_id && catalog?.routes_by_id.has(route_id)
			? { source: selected.source, route_id, at: current_time.value }
			: null;
	});
	const initial_service = $derived.by(() => {
		void page.state.modal;
		return untrack(() => selected_service.current.value);
	});
	$effect(() => {
		const selected = page.state.modal;
		if (!selected) return;
		// Capture cached membership before a cold catalog or service request completes.
		const cached = initial_service;
		const catalog = selected_catalog.current.value;
		// Keep the requested modal and its shared Retry available during catalog failures.
		if (!catalog) return;
		const service = selected_service.current;
		let invalid: boolean;
		switch (selected.type) {
			case 'stop':
				invalid = !catalog.stops_by_id.has(selected.id);
				break;
			case 'route':
				invalid = !catalog.routes_by_id.has(selected.id);
				break;
			case 'trip':
				invalid =
					!catalog.routes_by_id.has(selected.route_id) ||
					service.error instanceof UnknownRouteError ||
					(!!service.value &&
						(current_time.value !== undefined || service.value !== cached) &&
						!service.value.trips.has(selected.id));
				break;
		}
		if (invalid) untrack(() => close_modal(current_time.value, true));
	});

	const modal: Attachment<HTMLDivElement> = (node) => {
		// TODO: Restore native <dialog closedby="none"> with showModal() and remove
		// the custom backdrop/focus helper once this Firefox Android bug is fixed
		// in supported versions and consecutive system back gestures traverse history:
		// https://bugzilla.mozilla.org/show_bug.cgi?id=2078216
		// A native dialog registers a CloseWatcher even with closedby="none".
		// Firefox Android consumes back for that disabled watcher. Keep the sheet
		// in the page so history owns back, and supply modal focus/inertness here.

		// The focus attachment owns opening/restoration. Only refocus here when
		// replacing an already-open modal removes its focused content.
		let was_open = false;
		$effect(() => {
			void current_page_state.modal;
			if (was_open && has_modal && !node.contains(document.activeElement)) node.focus();
			was_open = has_modal;
		});

		// Add keyboard handler for Escape key
		function handle_keydown(event: KeyboardEvent) {
			if (has_modal && event.key === 'Escape') {
				event.preventDefault();
				close_modal(current_time.value);
			}
		}

		// Dismiss only a tap that starts and ends on the backdrop, not a drag.
		const delta = 6;
		let pointer_start: { id: number; x: number; y: number } | undefined;

		function handle_pointer_down(event: PointerEvent) {
			// The animated sheet is a snapshot with pointer events disabled, so a
			// rapid tap on it can hit the live backdrop underneath.
			pointer_start =
				!modal_transition && event.target === node.parentElement && event.button === 0
					? { id: event.pointerId, x: event.pageX, y: event.pageY }
					: undefined;
		}

		function handle_pointer_up(event: PointerEvent) {
			const start = pointer_start;
			pointer_start = undefined;
			if (
				modal_transition ||
				!start ||
				start.id !== event.pointerId ||
				event.target !== node.parentElement
			)
				return;
			const diffX = Math.abs(event.pageX - start.x);
			const diffY = Math.abs(event.pageY - start.y);

			if (diffX < delta && diffY < delta) {
				close_modal(current_time.value);
			}
		}

		const listeners_to_remove: Array<() => void> = [];

		listeners_to_remove.push(on(node.parentElement!, 'pointerdown', handle_pointer_down));
		listeners_to_remove.push(on(node.parentElement!, 'pointerup', handle_pointer_up));
		listeners_to_remove.push(
			on(node.parentElement!, 'pointercancel', () => {
				pointer_start = undefined;
			})
		);
		listeners_to_remove.push(on(document, 'keydown', handle_keydown));

		return () => {
			listeners_to_remove.forEach((off) => off());
		};
	};

	// const rotation = new Tween(0, {
	// 	duration: 300,
	// 	easing: cubicOut
	// });
	let copied = $state(false);
	// show stops/trips before current datetime TODO: maybe persist this preference in local storage
	let show_previous = $state(false);
	// e.g. 3m or 12:45.
	let time_format = new LocalStorage<'countdown' | 'time'>('time_format', 'countdown');
</script>

<!-- TODO: refactor actions now that we have sources -->

{#snippet actions(
	history: boolean,
	id: string,
	title: string,
	source: Source,
	pins: LocalStorage<Pins>
)}
	<div class="flex h-16 shrink-0 items-center justify-between gap-1 px-1">
		<button
			onclick={() => {
				close_modal(current_time.value);
			}}
			aria-label="Close modal"
			title="Close modal"
		>
			<CircleX size="2rem" />
		</button>

		<div class="flex items-center gap-1 text-xs">
			{#if history}
				<button
					class={{ 'text-muted': !show_previous, 'text-primary': show_previous }}
					aria-label="Show previous"
					onclick={() => {
						show_previous = !show_previous;
					}}
				>
					<RotateCcwClock size="2rem" />
				</button>
			{/if}

			<!-- <style>
				@keyframes spin-forward {
					from {
						transform: rotate(0deg);
					}
					to {
						transform: rotate(360deg);
					}
				}

				@keyframes spin-backward {
					from {
						transform: rotate(0deg);
					}
					to {
						transform: rotate(-360deg);
					}
				}

				.spin-forward {
					animation: spin-forward 0.3s linear;
				}

				.spin-backward {
					animation: spin-backward 0.3s linear;
				}
			</style> -->

			<button
				class="flex flex-col items-center"
				aria-label="Change time formatting"
				title="Change time formatting"
				onclick={() => {
					time_format.current = time_format.current === 'countdown' ? 'time' : 'countdown';
				}}
			>
				{#if time_format.current === 'countdown'}
					<AlarmClock size="2rem" />
				{:else}
					<Timer size="2rem" />
				{/if}
				<!-- Time Format -->
			</button>

			{#if !copied}
				<button
					aria-label="Share"
					title="Share"
					onclick={() => {
						// URL already includes ?s/?r/?t and ?at params via shallow routing
						const url = window.location.href;

						// Only use share api if on mobile and supported
						if (!navigator.share || !/Mobi/i.test(window.navigator.userAgent)) {
							navigator.clipboard.writeText(url);
							copied = true;
							setTimeout(() => {
								copied = false;
							}, 800);
						} else {
							navigator.share({
								title,
								url
							});
						}
					}}
				>
					<Share size="2rem" />
				</button>
			{:else}
				<button class="flex appearance-none text-accent" aria-label="Link copied to clipboard">
					<ClipboardCheck size="2rem" />
				</button>
			{/if}

			<Pin {id} {pins} {source} size="2rem" />
		</div>
	</div>
{/snippet}
<!-- fixed bottom-0 left-0 right-0 -->
<div class="modal-backdrop fixed inset-0 z-100 bg-backdrop" data-open={has_modal}>
	<div
		role="dialog"
		aria-modal="true"
		aria-label="Transit details"
		tabindex="-1"
		{@attach has_modal && contain_modal_focus}
		{@attach modal}
		class="modal-sheet absolute inset-x-0 bottom-0 m-auto flex max-h-[95dvh] w-full max-w-200 flex-col overflow-auto rounded-none bg-surface-1 text-white focus:ring-2 focus:ring-border-default focus:outline-hidden"
	>
		{#if has_modal}<UpdateIndicator compact />{/if}
		{#if selected_modal?.type === 'stop'}
			<StopModal
				source={selected_modal.source}
				stop_id={selected_modal.id}
				{show_previous}
				time_format={time_format.current}
			/>
			{@render actions(
				true,
				selected_modal.id,
				`Arrivals at ${catalog_of(selected_modal.source)?.stops_by_id.get(selected_modal.id)?.name ?? selected_modal.id}`,
				selected_modal.source,
				stop_pins
			)}
		{:else if selected_modal?.type === 'route'}
			<RouteModal
				source={selected_modal.source}
				route_id={selected_modal.id}
				time_format={time_format.current}
			/>
			{@render actions(
				false,
				selected_modal.id,
				`Alerts for ${catalog_of(selected_modal.source)?.routes_by_id.get(selected_modal.id)?.short_name ?? selected_modal.id}`,
				selected_modal.source,
				route_pins
			)}
		{:else if selected_modal?.type === 'trip'}
			<TripModal
				source={selected_modal.source}
				route_id={selected_modal.route_id}
				trip_id={selected_modal.id}
				{show_previous}
				time_format={time_format.current}
			/>
			{@render actions(
				true,
				trip_pin_key(selected_modal.route_id, selected_modal.id),
				`${selected_modal.route_id} Trip`,
				selected_modal.source,
				trip_pins
			)}
		{/if}
	</div>
</div>

<!-- <style>
	@keyframes spin {
		from {
			transform: rotate(0deg);
		}
		to {
			transform: rotate(360deg);
		}
	}

	.spin {
		animation: spin 0.5s linear;
	}
</style> -->
<style>
	.modal-backdrop[data-open='false'] {
		display: none;
	}

	.modal-backdrop[data-open='true'] .modal-sheet {
		view-transition-name: modal;
	}
</style>
