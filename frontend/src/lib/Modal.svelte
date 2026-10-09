<script lang="ts">
	import { flushSync, untrack } from 'svelte';

	import type { Attachment } from 'svelte/attachments';
	import { on } from 'svelte/events';

	import { page } from '$app/state';

	import type { Source } from '#lib/client/index.js';
	import { contain_modal_focus } from '#lib/modal_focus.js';
	import Pin from '#lib/Pin.svelte';
	import { route_pins, stop_pins, trip_pins, type Pins } from '#lib/pins.svelte.js';
	import RouteModal from '#lib/Route/Modal.svelte';
	import StopModal from '#lib/Stop/Modal.svelte';
	import { LocalStorage } from '#lib/storage.svelte.js';
	import TripModal from '#lib/Trip/Modal.svelte';
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
		return next_modal.type !== local_modal.type || next_modal.id !== local_modal.id;
	}

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
			transition.ready.catch(() => {});
			transition.finished.catch(() => {});
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
			pointer_start =
				event.target === node.parentElement && event.button === 0
					? { id: event.pointerId, x: event.pageX, y: event.pageY }
					: undefined;
		}

		function handle_pointer_up(event: PointerEvent) {
			const start = pointer_start;
			pointer_start = undefined;
			if (!start || start.id !== event.pointerId || event.target !== node.parentElement) return;
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

			<Pin {id} {pins} {source} size="2rem" />
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
		{#if current_page_state.modal?.type === 'stop'}
			<StopModal
				stop={current_page_state.modal}
				{show_previous}
				time_format={time_format.current}
			/>

			{@render actions(
				true,
				current_page_state.modal.id,
				`Arrivals at ${current_page_state.modal.name}`,
				current_page_state.modal.data.source,
				stop_pins
			)}
		{:else if current_page_state.modal?.type === 'route'}
			<RouteModal route={current_page_state.modal} time_format={time_format.current} />

			{@render actions(
				false,
				current_page_state.modal.id,
				`Alerts for ${current_page_state.modal.short_name}`,
				current_page_state.modal.data.source,
				route_pins
			)}
		{:else if current_page_state.modal?.type === 'trip'}
			<TripModal
				trip={current_page_state.modal}
				{show_previous}
				time_format={time_format.current}
			/>

			{@render actions(
				true,
				current_page_state.modal.id,
				`${current_page_state.modal.route_id} Trip`,
				current_page_state.modal.data.source,
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
