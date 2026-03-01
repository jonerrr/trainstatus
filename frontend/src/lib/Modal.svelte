<script lang="ts">
	import type { Attachment } from 'svelte/attachments';
	import { on } from 'svelte/events';

	import { page } from '$app/state';

	import Pin from '$lib/Pin.svelte';
	import RouteModal from '$lib/Route/Modal.svelte';
	import SettingsModal from '$lib/Settings/Modal.svelte';
	import StopModal from '$lib/Stop/Modal.svelte';
	import TripModal from '$lib/Trip/Modal.svelte';
	import { type Pins, route_pins, stop_pins, trip_pins } from '$lib/pins.svelte';
	import { LocalStorage } from '$lib/storage.svelte';
	import { close_modal } from '$lib/url_params.svelte';

	import { AlarmClock, CircleX, ClipboardCheck, History, Share, Timer } from '@lucide/svelte';
	import type { Source } from '@trainstatus/client';

	// import { type Trip, type TripData, is_bus_route } from './trips.svelte';

	// import { Tween } from 'svelte/motion';
	// import { cubicOut } from 'svelte/easing';

	// interface Props {
	// 	current_time?: number;
	// }

	// const { current_time }: Props = $props();

	// let dialog_el = $state<HTMLDialogElement>();

	// TODO: make implement some sort of focus trap and restore using attachments
	// see https://svelte.dev/tutorial/svelte/attach

	// Physics constants derived from vaul-svelte
	const VELOCITY_THRESHOLD = 0.4; // px/ms
	const CLOSE_THRESHOLD = 0.25; // % of height
	const DRAG_RESISTANCE = 8; // Rubber band strength
	const DRAG_START_THRESHOLD = 5; // px of movement before we consider it a real drag

	// State for drag physics
	let is_dragging = $state(false);
	let translate_y = $state(0);
	let start_y = 0;
	let start_time = 0;
	let modal_height = 0;
	let is_transitioning = $state(false);
	let has_captured = false;

	// Derived style for the modal
	// We disable CSS transitions while dragging so it feels responsive (1:1 movement)
	let modal_style = $derived(
		`transform: translate3d(0, ${translate_y}px, 0); ` +
			`transition: ${is_transitioning ? 'transform 0.3s cubic-bezier(0.32, 0.72, 0, 1)' : 'none'};` +
			// Important: prevent browser gestures like "back" or "refresh" while dragging
			(is_dragging ? 'touch-action: none;' : '')
	);

	// Logarithmic dampening for "rubber banding" (dragging upwards past 0)
	function dampen_value(v: number) {
		return DRAG_RESISTANCE * (Math.log(v + 1) - 2);
	}

	function close() {
		close_modal();
	}
	// TODO: improve physics on desktop (or maybe just disable)
	const modal: Attachment<HTMLDialogElement> = (node) => {
		document.body.style.overflow = 'hidden';

		$effect(() => {
			// TODO: why does this run twice on close
			console.log(`modal update`, { state: page.state });
			if (page.state.modal) {
				node.showModal();
			} else {
				node.close();
			}
		});

		// TODO: maybe need to handle when dialog has long scrollable content
		// 		function handle_pointer_down(e: PointerEvent) {
		//     // Check if target is inside a scrollable element that isn't at the top
		//     let target = e.target as HTMLElement;
		//     while (target && target !== element) {
		//         if (target.scrollHeight > target.clientHeight) {
		//             // It is scrollable. If we are not at the top, don't drag the modal.
		//             if (target.scrollTop > 0) return;
		//         }
		//         target = target.parentElement as HTMLElement;
		//     }

		//     // ... rest of the function
		//     element.setPointerCapture(e.pointerId);
		// }

		function handle_pointer_down(e: PointerEvent) {
			// Ignore if clicking a button or specific non-draggable areas if needed
			// if ((e.target as HTMLElement).closest('button')) return;

			is_dragging = true;
			has_captured = false;
			is_transitioning = false;
			start_y = e.screenY;
			start_time = Date.now();
			modal_height = node.getBoundingClientRect().height;

			// Don't call setPointerCapture here — doing so on every pointerdown (including
			// taps on inner content) causes the browser to redirect the synthesized click
			// event to the dialog element, which triggers handle_click and closes the modal.
			// Instead we capture lazily in handle_pointer_move once a real drag begins.
		}

		function handle_pointer_move(e: PointerEvent) {
			if (!is_dragging) return;

			// Lazily capture pointer once we know it's a real drag, not a tap
			if (!has_captured && Math.abs(e.screenY - start_y) > DRAG_START_THRESHOLD) {
				node.setPointerCapture(e.pointerId);
				has_captured = true;
			}

			const delta_y = e.screenY - start_y;

			// If dragging down (positive delta), move 1:1
			if (delta_y > 0) {
				translate_y = delta_y;
			}
			// If dragging up (negative delta), apply resistance (dampening)
			else {
				translate_y = dampen_value(Math.abs(delta_y)) * -1;
			}
		}

		function handle_pointer_up(e: PointerEvent) {
			if (!is_dragging) return;

			is_dragging = false;
			is_transitioning = true; // Re-enable CSS transitions for the snap back/close
			if (has_captured) {
				node.releasePointerCapture(e.pointerId);
				has_captured = false;
			}

			const end_y = e.screenY;
			const distance = end_y - start_y;
			const time_taken = Date.now() - start_time;
			const velocity = Math.abs(distance) / time_taken;

			// 1. Velocity Check (Flick)
			// Only close on flick if moving downwards
			if (distance > 0 && velocity > VELOCITY_THRESHOLD) {
				close_animate();
				return;
			}

			// 2. Threshold Check (Drag distance)
			// Close if dragged down more than 25% of height
			if (distance > 0 && distance > modal_height * CLOSE_THRESHOLD) {
				close_animate();
				return;
			}

			// 3. Reset (Snap back)
			// If neither condition met, bounce back to 0
			translate_y = 0;
		}

		function close_animate() {
			// Animate off screen
			translate_y = modal_height; // Slide completely out of view

			// Wait for animation to finish before actually closing state
			setTimeout(() => {
				close();
				// Reset position for next open (though component usually remounts)
				translate_y = 0;
			}, 300);
		}

		// watch for clicks outside the dialog to close it
		function handle_click(event: MouseEvent) {
			if (event.target === node) {
				close();
			}
		}

		// Add keyboard handler for Escape key
		function handle_keydown(event: KeyboardEvent) {
			if (event.key === 'Escape') {
				event.preventDefault();
				close();
			}
		}

		// This differentiates between a drag and a click so mobile users don't accidentally close the dialog when swiping to go back
		// from here https://stackoverflow.com/a/59741870
		const delta = 6;
		let startX: number;
		let startY: number;

		function handle_mouse_down(event: MouseEvent) {
			if (event.target !== node) return;

			startX = event.pageX;
			startY = event.pageY;
		}

		function handle_mouse_up(event: MouseEvent) {
			// Only act if the mouse started and ended directly on the dialog node
			if (event.target !== node) return;

			const diffX = Math.abs(event.pageX - startX);
			const diffY = Math.abs(event.pageY - startY);

			if (diffX < delta && diffY < delta) {
				close();
			}
		}

		const listeners_to_remove: Array<() => void> = [];

		listeners_to_remove.push(on(node, 'click', handle_click));
		listeners_to_remove.push(on(node, 'mousedown', handle_mouse_down));
		listeners_to_remove.push(on(node, 'mouseup', handle_mouse_up));
		listeners_to_remove.push(on(document, 'keydown', handle_keydown));

		// physics events
		listeners_to_remove.push(on(node, 'pointerdown', handle_pointer_down));
		listeners_to_remove.push(on(node, 'pointermove', handle_pointer_move));
		listeners_to_remove.push(on(node, 'pointerup', handle_pointer_up));
		listeners_to_remove.push(on(node, 'pointercancel', handle_pointer_up)); // Handle cases where the pointer is canceled (e.g., system interrupts)

		return () => {
			document.body.style.overflow = '';
			listeners_to_remove.forEach((off) => off());
		};
	};

	// TODO: use runed url search param management
	// manage title changes, dialog el, and monitored bus routes
	// $effect(() => {
	// 	// console.log('modal effect');
	// 	switch (page.state.modal) {
	// 		case 'route':
	// 			dialog_el?.showModal();
	// 			document.title = `${page.state.data?.id} Alerts | Train Status`;
	// 			break;
	// 		case 'stop':
	// 			dialog_el?.showModal();
	// 			document.title = `${(page.state.data as Stop)?.name} | Train Status`;

	// 			const stop: Stop = page.state.data;
	// 			// TODO: add back
	// 			// if (is_bus(stop)) {
	// 			// 	// console.log('monitoring modal bus routes');
	// 			// 	stop.routes.forEach((r) => monitored_bus_routes.add(r.id));
	// 			// }
	// 			break;
	// 		case 'trip':
	// 			dialog_el?.showModal();
	// 			document.title = `${page.state.data?.route_id} Trip | Train Status`;

	// 			const trip: Trip = page.state.data;
	// 			const bus_route = page.data.routes[trip.route_id];
	// 			// if (is_bus_route(bus_route, trip)) {
	// 			// 	// console.log('monitoring modal bus routes');
	// 			// 	monitored_bus_routes.add(trip.route_id);
	// 			// }

	// 			break;
	// 		case 'settings':
	// 			dialog_el?.showModal();
	// 			document.title = 'Settings | Train Status';
	// 			break;
	// 		default:
	// 			dialog_el?.close();
	// 			switch (page.route.id) {
	// 				case '/stops':
	// 					document.title = 'Stops | Train Status';
	// 					break;
	// 				case '/alerts':
	// 					document.title = 'Alerts | Train Status';
	// 					break;
	// 				case '/charts':
	// 					document.title = 'Charts | Train Status';
	// 					break;
	// 				default:
	// 					document.title = 'Home | Train Status';
	// 					break;
	// 			}
	// 			break;
	// 	}
	// });
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
	param_name: 'r' | 's' | 't',
	id: string,
	title: string,
	source: Source,
	pins: LocalStorage<Pins>
)}
	<div class="flex h-16 items-center justify-between gap-1 px-1">
		<button
			onclick={() => {
				close();
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
					<History size="2rem" />
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
<dialog
	{@attach modal}
	style={modal_style}
	class="m-auto mb-0 flex max-h-[95dvh] w-full max-w-200 flex-col rounded-t-sm bg-neutral-900 text-white backdrop:bg-black/50 focus:ring-2 focus:ring-neutral-700 focus:outline-hidden"
>
	{#if page.state.modal?.type === 'stop'}
		<StopModal stop={page.state.modal} {show_previous} time_format={time_format.current} />

		{@render actions(
			true,
			's',
			page.state.modal.id,
			`Arrivals at ${page.state.modal.name}`,
			page.state.modal.data.source,
			stop_pins
		)}
	{:else if page.state.modal?.type === 'route'}
		<RouteModal route={page.state.modal} time_format={time_format.current} />

		{@render actions(
			false,
			'r',
			page.state.modal.id,
			`Alerts for ${page.state.modal.short_name}`,
			page.state.modal.data.source,
			route_pins
		)}
	{:else if page.state.modal?.type === 'trip'}
		<TripModal trip={page.state.modal} {show_previous} time_format={time_format.current} />

		{@render actions(
			true,
			't',
			page.state.modal.id,
			`${page.state.modal.route_id} Trip`,
			page.state.modal.data.source,
			trip_pins
		)}
	{:else if page.state.modal?.type === 'settings'}
		<SettingsModal />
	{/if}
</dialog>

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
	dialog[open] {
		view-transition-name: modal;
	}

	dialog::backdrop {
		background-color: rgb(0 0 0 / 50%);
	}
</style>
