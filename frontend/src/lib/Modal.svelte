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
	import { close_modal, getCurrentTime, type ModalData } from '#lib/url_params.svelte.js';

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
	let current_page_state = $state(page.state);
	const has_modal = $derived(!!current_page_state.modal);

	// $state proxies and the objects they wrap are not ===. Compare the modal
	// itself, or assigning page.state back into current_page_state retriggers
	// the effect and starts another view transition on every flush.
	function modal_changed(next: ModalData | null | undefined, local: ModalData | null | undefined) {
		const next_modal = $state.snapshot(next);
		const local_modal = $state.snapshot(local);
		if (!next_modal || !local_modal) return !!next_modal !== !!local_modal;
		return (
			next_modal.type !== local_modal.type ||
			next_modal.id !== local_modal.id ||
			next_modal.source !== local_modal.source ||
			(next_modal.type === 'trip' &&
				local_modal.type === 'trip' &&
				next_modal.route_id !== local_modal.route_id)
		);
	}

	let modal_transition: ViewTransition | undefined;

	function slide_to(next_state: typeof page.state, forward: boolean) {
		document.documentElement.dataset.modalDirection = forward ? 'forward' : 'backward';

		const apply = () => {
			current_page_state = $state.snapshot(next_state);
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
		if (untrack(() => modal_changed(next_state.modal, current_page_state.modal))) {
			const forward = untrack(() => (next_state.index ?? 0) > (current_page_state?.index ?? 0));
			slide_to(next_state, forward);
		}
	});

	const selected_catalog = transit.catalog(() => page.state.modal?.source ?? null);
	const selected_service = transit.route(() => {
		const selected = page.state.modal;
		const catalog = selected_catalog.current.value;
		const route_id =
			selected?.type === 'route'
				? selected.id
				: selected?.type === 'trip'
					? selected.route_id
					: null;
		return selected && route_id && catalog?.routes_by_id.has(route_id)
			? { source: selected.source, route_id, at: current_time.value }
			: null;
	});
	$effect(() => {
		const selected = page.state.modal;
		if (!selected) return;
		const catalog = selected_catalog.current.value;
		if (!catalog) return; // Failed or pending catalog reads cannot establish validity.
		const service = selected_service.current;
		const invalid =
			selected.type === 'stop'
				? !catalog.stops_by_id.has(selected.id)
				: selected.type === 'route'
					? !catalog.routes_by_id.has(selected.id) || service.error instanceof UnknownRouteError
					: !selected.route_id ||
						!catalog.routes_by_id.has(selected.route_id) ||
						service.error instanceof UnknownRouteError ||
						(!service.loading &&
							!service.error &&
							service.value !== undefined &&
							!service.value.trips.has(selected.id));
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
	pins: LocalStorage<Pins> | undefined
)}
	<div class="flex h-16 items-center justify-between gap-1 px-1">
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
					class:text-neutral-400={!show_previous}
					class:text-neutral-50={show_previous}
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
				<button class="flex appearance-none text-green-600" aria-label="Link copied to clipboard">
					<ClipboardCheck size="2rem" />
				</button>
			{/if}

			{#if pins}
				<Pin {id} {pins} {source} size="2rem" />
			{/if}
		</div>
	</div>
{/snippet}
<!-- fixed bottom-0 left-0 right-0 -->
<div class="modal-backdrop fixed inset-0 z-100 bg-black/50" data-open={has_modal}>
	<div
		role="dialog"
		aria-modal="true"
		aria-label="Transit details"
		tabindex="-1"
		{@attach has_modal && contain_modal_focus}
		{@attach modal}
		class="modal-sheet absolute inset-x-0 bottom-0 m-auto flex max-h-[95dvh] w-full max-w-200 flex-col overflow-auto rounded-t-sm bg-neutral-900 text-white focus:ring-2 focus:ring-neutral-700 focus:outline-hidden"
	>
		{#if has_modal}<UpdateIndicator compact />{/if}
		{#if current_page_state.modal?.type === 'stop'}
			{const { source, id } = $derived(current_page_state.modal)}
			<StopModal {source} stop_id={id} {show_previous} time_format={time_format.current} />

			{@render actions(
				true,
				id,
				`Arrivals at ${catalog_of(source)?.stops_by_id.get(id)?.name ?? id}`,
				source,
				stop_pins
			)}
		{:else if current_page_state.modal?.type === 'route'}
			{const { source, id } = $derived(current_page_state.modal)}
			<RouteModal {source} route_id={id} time_format={time_format.current} />

			{@render actions(
				false,
				id,
				`Alerts for ${catalog_of(source)?.routes_by_id.get(id)?.short_name ?? id}`,
				source,
				route_pins
			)}
		{:else if current_page_state.modal?.type === 'trip'}
			{const { source, id, route_id } = $derived(current_page_state.modal)}
			<TripModal
				{source}
				{route_id}
				trip_id={id}
				{show_previous}
				time_format={time_format.current}
			/>

			{@render actions(
				true,
				route_id ? trip_pin_key(route_id, id) : id,
				`${route_id ?? ''} Trip`,
				source,
				route_id ? trip_pins : undefined
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
