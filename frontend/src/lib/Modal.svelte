<script lang="ts">
	import type { Attachment } from 'svelte/attachments';

	import { pushState } from '$app/navigation';
	import { page } from '$app/state';

	import Pin from '$lib/Pin.svelte';
	import RouteModal from '$lib/Route/Modal.svelte';
	import SettingsModal from '$lib/Settings/Modal.svelte';
	import StopModal from '$lib/Stop/Modal.svelte';
	import TripModal from '$lib/Trip/Modal.svelte';
	import { type Pins, route_pins, stop_pins } from '$lib/stores.svelte';
	import { current_time } from '$lib/util.svelte';

	import { AlarmClock, CircleX, ClipboardCheck, History, Share, Timer } from '@lucide/svelte';
	import type { Source, Stop, Trip, TripData } from '@trainstatus/client';
	import { PersistedState } from 'runed';

	// import { type Trip, type TripData, is_bus_route } from './trips.svelte';

	// import { Tween } from 'svelte/motion';
	// import { cubicOut } from 'svelte/easing';

	// interface Props {
	// 	current_time?: number;
	// }

	// const { current_time }: Props = $props();

	let dialog_el = $state<HTMLDialogElement>();

	function close() {
		// enable_scroll();
		pushState('', { modal: null });
	}

	const modal: Attachment<HTMLDialogElement> = (element) => {
		document.body.style.overflow = 'hidden';

		// watch for clicks outside the dialog to close it
		function handle_click(event: MouseEvent) {
			if (event.target === element) {
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
			if (event.target !== element) return;

			startX = event.pageX;
			startY = event.pageY;
		}

		function handle_mouse_up(event: MouseEvent) {
			// Only act if the mouse started and ended directly on the dialog element
			if (event.target !== element) return;

			const diffX = Math.abs(event.pageX - startX);
			const diffY = Math.abs(event.pageY - startY);

			if (diffX < delta && diffY < delta) {
				close();
			}
		}

		element.addEventListener('click', handle_click);
		element.addEventListener('mousedown', handle_mouse_down);
		element.addEventListener('mouseup', handle_mouse_up);
		document.addEventListener('keydown', handle_keydown);

		return () => {
			document.body.style.overflow = '';
			element.removeEventListener('mousedown', handle_mouse_down);
			element.removeEventListener('mouseup', handle_mouse_up);
			element.removeEventListener('click', handle_click);
			document.removeEventListener('keydown', handle_keydown);
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
	// TODO: use debounced from runed here
	let copied = $state(false);
	// show stops/trips before current datetime
	let show_previous = $state(false);
	// let time_format = persisted_rune<'countdown' | 'time'>('time_format', 'countdown');
	let time_format = new PersistedState<'countdown' | 'time'>('time_format', 'countdown');
</script>

<!-- TODO: refactor actions now that we have sources -->
{#snippet actions(
	history: boolean,
	param_name: 'r' | 's' | 't',
	id: string,
	title: string,
	source: Source,
	pins: PersistedState<Pins>
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
						const url = `${window.location.origin}/?${param_name}=${id}${current_time.value ? `&at=${current_time.value}` : ''}`;

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
	bind:this={dialog_el}
	class="m-auto mb-0 flex max-h-[95dvh] w-full max-w-200 flex-col rounded-t-sm bg-neutral-900 text-white backdrop:bg-black/50 focus:ring-2 focus:ring-neutral-700 focus:outline-hidden"
>
	{#if page.state.modal?.type === 'stop'}
		<StopModal {show_previous} time_format={time_format.current} stop={page.state.modal.data} />

		{@render actions(
			true,
			's',
			page.state.modal.data.id,
			`Arrivals at ${page.state.modal.data.name}`,
			page.state.modal.source,
			stop_pins
		)}
	{:else if page.state.modal?.type === 'route'}
		<RouteModal route={page.state.modal.data} time_format={time_format.current} />

		{@render actions(
			false,
			'r',
			page.state.modal.data.id,
			`Alerts for ${page.state.modal.data.id}`,
			page.state.modal.source,
			route_pins
		)}
	{:else if page.state.modal?.type === 'trip'}
		<!-- <TripModal trip={page.state.modal.data} {show_previous} time_format={time_format.current} /> -->
		<!-- TODO: trips -->
		<!-- {@render actions(
			true,
			't',
			page.state.modal.data.id,
			`${page.state.modal.data.route_id} Trip`,
			trip_pins
		)} -->
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
