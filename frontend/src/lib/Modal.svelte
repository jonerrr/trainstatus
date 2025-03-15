<script lang="ts">
	import { CircleX, Share, ClipboardCheck, History, Timer, AlarmClock } from 'lucide-svelte';
	import { pushState } from '$app/navigation';
	import { page } from '$app/state';
	import {
		stop_pins_rune,
		trip_pins_rune,
		route_pins_rune,
		type PersistedRune,
		persisted_rune,
		current_time
	} from '$lib/util.svelte';
	import { is_bus, type Stop } from './static';
	import { monitored_bus_routes } from './stop_times.svelte';
	import StopModal from '$lib/Stop/Modal.svelte';
	import TripModal from '$lib/Trip/Modal.svelte';
	import RouteModal from '$lib/Route/Modal.svelte';
	import SettingsModal from '$lib/Settings/Modal.svelte';
	import Pin from './Pin.svelte';
	import { is_bus_route, type Trip, type TripData } from './trips.svelte';
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

	function manage_modal(node: HTMLDialogElement) {
		document.body.style.overflow = 'hidden';

		// watch for clicks outside the dialog to close it
		function handle_click(event: MouseEvent) {
			if (event.target === node) {
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
			// Only act if the mouse started and ended directly on the dialog element
			if (event.target !== node) return;

			const diffX = Math.abs(event.pageX - startX);
			const diffY = Math.abs(event.pageY - startY);

			if (diffX < delta && diffY < delta) {
				close();
			}
		}

		node.addEventListener('click', handle_click);
		node.addEventListener('mousedown', handle_mouse_down);
		node.addEventListener('mouseup', handle_mouse_up);

		return {
			destroy() {
				document.body.style.overflow = '';
				node.removeEventListener('mousedown', handle_mouse_down);
				node.removeEventListener('mouseup', handle_mouse_up);
				node.removeEventListener('click', handle_click);
			}
		};
	}

	// manage title changes, dialog el, and monitored bus routes
	$effect(() => {
		// console.log('modal effect');
		switch (page.state.modal) {
			case 'route':
				dialog_el?.showModal();
				document.title = `${page.state.data.id} Alerts | Train Status`;
				break;
			case 'stop':
				dialog_el?.showModal();
				document.title = `${page.state.data.name} | Train Status`;

				const stop: Stop<'bus' | 'train'> = page.state.data;
				if (is_bus(stop)) {
					// console.log('monitoring modal bus routes');
					stop.routes.forEach((r) => monitored_bus_routes.add(r.id));
				}
				break;
			case 'trip':
				dialog_el?.showModal();
				document.title = `${page.state.data.route_id} Trip | Train Status`;

				const trip: Trip<TripData> = page.state.data;
				const bus_route = page.data.routes[trip.route_id];
				if (is_bus_route(bus_route, trip)) {
					// console.log('monitoring modal bus routes');
					monitored_bus_routes.add(trip.route_id);
				}

				break;
			case 'settings':
				dialog_el?.showModal();
				document.title = 'Settings | Train Status';
				break;
			default:
				dialog_el?.close();
				switch (page.route.id) {
					case '/stops':
						document.title = 'Stops | Train Status';
						break;
					case '/alerts':
						document.title = 'Alerts | Train Status';
						break;
					default:
						document.title = 'Home | Train Status';
						break;
				}
				break;
		}
	});
	// const rotation = new Tween(0, {
	// 	duration: 300,
	// 	easing: cubicOut
	// });

	let copied = $state(false);
	// show stops/trips before current datetime
	let show_previous = $state(false);
	let time_format = persisted_rune<'countdown' | 'time'>('time_format', 'countdown');
</script>

{#snippet actions(
	history: boolean,
	param_name: 'r' | 's' | 't',
	id: string | number,
	title: string,
	pin_rune: PersistedRune<(string | number)[]>
)}
	<div class="flex gap-1 items-center justify-between px-1 h-16">
		<button
			onclick={() => {
				close();
			}}
			aria-label="Close modal"
			title="Close modal"
		>
			<CircleX size="2rem" />
		</button>

		<div class="flex gap-1 items-center text-xs">
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
					time_format.value = time_format.value === 'countdown' ? 'time' : 'countdown';
				}}
			>
				{#if time_format.value === 'countdown'}
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
				<button class="appearance-none flex text-green-600" aria-label="Link copied to clipboard">
					<ClipboardCheck size="2rem" />
				</button>
			{/if}

			<Pin {id} {pin_rune} size="2rem" />
		</div>
	</div>
{/snippet}
<!-- fixed bottom-0 left-0 right-0 -->
<dialog
	bind:this={dialog_el}
	use:manage_modal
	class="m-auto text-white bg-neutral-900 w-full max-w-[800px] max-h-[95dvh] rounded-t-sm flex flex-col backdrop:bg-black/50 mb-0 focus:outline-hidden focus:ring-2 focus:ring-neutral-700"
>
	{#if page.state.modal === 'stop'}
		<StopModal {show_previous} time_format={time_format.value} stop={page.state.data} />

		{@render actions(
			true,
			's',
			page.state.data.id,
			`Arrivals at ${page.state.data.name}`,
			stop_pins_rune
		)}
	{:else if page.state.modal === 'route'}
		<RouteModal route={page.state.data} time_format={time_format.value} />

		{@render actions(
			false,
			'r',
			page.state.data.id,
			`Alerts for ${page.state.data.id}`,
			route_pins_rune
		)}
	{:else if page.state.modal === 'trip'}
		<TripModal trip={page.state.data} {show_previous} time_format={time_format.value} />

		{@render actions(
			true,
			't',
			page.state.data.id,
			`${page.state.data.route_id} Trip`,
			trip_pins_rune
		)}
	{:else if page.state.modal === 'settings'}
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
