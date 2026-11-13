<script lang="ts">
	import { flushSync, untrack } from 'svelte';

	import type { Attachment } from 'svelte/attachments';
	import { on } from 'svelte/events';

	import { page } from '$app/state';

	import type { Source } from '#lib/client/index.js';
	import { getTransitData } from '#lib/data/store.svelte.js';
	import { containModalFocus } from '#lib/modal-focus.js';
	import Pin from '#lib/Pin.svelte';
	import { routePins, stopPins, tripPinKey, tripPins, type Pins } from '#lib/pins.svelte.js';
	import RouteModal from '#lib/route/Modal.svelte';
	import StopModal from '#lib/stop/Modal.svelte';
	import { LocalStorage } from '#lib/storage.svelte.js';
	import TripModal from '#lib/trip/Modal.svelte';
	import UpdateIndicator from '#lib/UpdateIndicator.svelte';
	import { closeModal, getCurrentTime } from '#lib/url-params.svelte.js';

	import {
		AlarmClock,
		CircleX,
		ClipboardCheck,
		RotateCcwClock,
		Share,
		Timer
	} from '@lucide/svelte';

	const currentTime = getCurrentTime();
	const transit = getTransitData();

	function catalogOf(source: Source) {
		return transit.peekCatalog(source);
	}

	// by reassigning the page.state locally, we can ensure the dialog transitions run before the DOM updates.
	// Otherwise, the sliding animation looks like it runs twice.
	let currentPageState = $state.raw(page.state);
	const selectedModal = $derived(currentPageState.modal);
	const hasModal = $derived(!!currentPageState.modal);

	// Raw state preserves modal identity across shallow navigation and animation snapshots.

	let modalTransition: ViewTransition | undefined;

	function slideTo(forward: boolean) {
		document.documentElement.dataset.modalDirection = forward ? 'forward' : 'backward';

		const apply = () => {
			// Catalog validation or another navigation can change the selection while
			// the transition waits to capture its old view. Apply the latest selection.
			currentPageState = page.state;
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
			modalTransition = transition;
			transition.ready.catch(() => {});
			const clearTransition = () => {
				if (modalTransition === transition) modalTransition = undefined;
			};
			transition.finished.then(clearTransition, clearTransition);
		} else {
			apply();
		}
	}

	$effect(() => {
		// $inspect.trace('modal state transition effect');
		const nextState = page.state;
		if (untrack(() => nextState.modal !== currentPageState.modal)) {
			const forward = untrack(() => (nextState.index ?? 0) > (currentPageState?.index ?? 0));
			slideTo(forward);
		}
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
		let wasOpen = false;
		$effect(() => {
			void currentPageState.modal;
			if (wasOpen && hasModal && !node.contains(document.activeElement)) node.focus();
			wasOpen = hasModal;
		});

		// Add keyboard handler for Escape key
		function handleKeydown(event: KeyboardEvent) {
			if (hasModal && event.key === 'Escape') {
				event.preventDefault();
				closeModal(currentTime.value);
			}
		}

		// Dismiss only a tap that starts and ends on the backdrop, not a drag.
		const delta = 6;
		let pointerStart: { id: number; x: number; y: number } | undefined;

		function handlePointerDown(event: PointerEvent) {
			// The animated sheet is a snapshot with pointer events disabled, so a
			// rapid tap on it can hit the live backdrop underneath.
			pointerStart =
				!modalTransition && event.target === node.parentElement && event.button === 0
					? { id: event.pointerId, x: event.pageX, y: event.pageY }
					: undefined;
		}

		function handlePointerUp(event: PointerEvent) {
			const start = pointerStart;
			pointerStart = undefined;
			if (
				modalTransition ||
				!start ||
				start.id !== event.pointerId ||
				event.target !== node.parentElement
			)
				return;
			const diffX = Math.abs(event.pageX - start.x);
			const diffY = Math.abs(event.pageY - start.y);

			if (diffX < delta && diffY < delta) {
				closeModal(currentTime.value);
			}
		}

		const listenersToRemove: Array<() => void> = [];

		listenersToRemove.push(on(node.parentElement!, 'pointerdown', handlePointerDown));
		listenersToRemove.push(on(node.parentElement!, 'pointerup', handlePointerUp));
		listenersToRemove.push(
			on(node.parentElement!, 'pointercancel', () => {
				pointerStart = undefined;
			})
		);
		listenersToRemove.push(on(document, 'keydown', handleKeydown));

		return () => {
			listenersToRemove.forEach((off) => off());
		};
	};

	// const rotation = new Tween(0, {
	// 	duration: 300,
	// 	easing: cubicOut
	// });
	let copied = $state(false);
	// show stops/trips before current datetime TODO: maybe persist this preference in local storage
	let showPrevious = $state(false);
	// e.g. 3m or 12:45.
	let timeFormat = new LocalStorage<'countdown' | 'time'>('timeFormat', 'countdown');
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
				closeModal(currentTime.value);
			}}
			aria-label="Close modal"
			title="Close modal"
		>
			<CircleX size="2rem" />
		</button>

		<div class="flex items-center gap-1 text-xs">
			{#if history}
				<button
					class={{ 'text-muted': !showPrevious, 'text-primary': showPrevious }}
					aria-label="Show previous"
					onclick={() => {
						showPrevious = !showPrevious;
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
					timeFormat.current = timeFormat.current === 'countdown' ? 'time' : 'countdown';
				}}
			>
				{#if timeFormat.current === 'countdown'}
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
<div class="modal-backdrop fixed inset-0 z-100 bg-backdrop" data-open={hasModal}>
	<div
		role="dialog"
		aria-modal="true"
		aria-label="Transit details"
		tabindex="-1"
		{@attach hasModal && containModalFocus}
		{@attach modal}
		class="modal-sheet absolute inset-x-0 bottom-0 m-auto flex max-h-[95dvh] w-full max-w-200 flex-col overflow-auto rounded-none bg-surface-1 text-white focus:ring-2 focus:ring-border-default focus:outline-hidden"
	>
		{#if hasModal}<UpdateIndicator compact />{/if}
		{#if selectedModal?.type === 'stop'}
			<StopModal
				source={selectedModal.source}
				stopId={selectedModal.id}
				{showPrevious}
				timeFormat={timeFormat.current}
			/>
			{@render actions(
				true,
				selectedModal.id,
				`Arrivals at ${catalogOf(selectedModal.source)?.stopsById.get(selectedModal.id)?.name ?? selectedModal.id}`,
				selectedModal.source,
				stopPins
			)}
		{:else if selectedModal?.type === 'route'}
			<RouteModal
				source={selectedModal.source}
				routeId={selectedModal.id}
				timeFormat={timeFormat.current}
			/>
			{@render actions(
				false,
				selectedModal.id,
				`Alerts for ${catalogOf(selectedModal.source)?.routesById.get(selectedModal.id)?.short_name ?? selectedModal.id}`,
				selectedModal.source,
				routePins
			)}
		{:else if selectedModal?.type === 'trip'}
			<TripModal
				source={selectedModal.source}
				routeId={selectedModal.routeId}
				tripId={selectedModal.id}
				{showPrevious}
				timeFormat={timeFormat.current}
			/>
			{@render actions(
				true,
				tripPinKey(selectedModal.routeId, selectedModal.id),
				`${selectedModal.routeId} Trip`,
				selectedModal.source,
				tripPins
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
