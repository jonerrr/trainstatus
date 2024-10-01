<script lang="ts">
	import { CircleX, Share, ClipboardCheck, CircleHelp, Dices, History } from 'lucide-svelte';
	import { type Writable } from 'svelte/store';
	import { page } from '$app/stores';
	import { pushState, replaceState } from '$app/navigation';
	import { all_route_ids } from '$lib/api';
	import {
		trips,
		stops,
		bus_stops,
		bus_routes,
		bus_trips,
		pinned_bus_stops,
		pinned_bus_trips,
		pinned_routes,
		pinned_bus_routes,
		pinned_stops,
		pinned_trips,
		monitored_routes
	} from '$lib/stores';
	import StopContent from '$lib/components/Stop/Content.svelte';
	import TripContent from '$lib/components/Trip/Content.svelte';
	import RouteAlertContent from '$lib/components/RouteAlert/Content.svelte';
	import BusStopContent from '$lib/components/Stop/BusContent.svelte';
	import BusTripContent from '$lib/components/Trip/BusContent.svelte';
	import Pin from '$lib/components/Pin.svelte';
	// import PastStops from '$lib/components/Trip/PastStops.svelte';

	// detect if user is swiping back and disable close on outside click

	let dialog_el: HTMLDialogElement;

	function manage_dialog(node: HTMLDialogElement) {
		page.subscribe((p) => {
			if (p.state.dialog_open) {
				// prevent close state issues
				node.close();
				// this prevents auto focusing on close button when opening dialog
				node.inert = true;
				node.showModal();
				node.inert = false;

				// prevent scrolling when dialog is open
				document.body.style.overflow = 'hidden';
			} else {
				node.close();
				document.body.style.overflow = 'auto';
				document.title = 'Trainstat.us';
			}
		});

		// This differentiates between a drag and a click so mobile users don't accidentally close the dialog when swiping to go back
		// from here https://stackoverflow.com/a/59741870
		const delta = 6;
		let startX: number;
		let startY: number;

		node.addEventListener('mousedown', function (event) {
			// Make sure the user is clicking outside of the dialog
			if (event.target === node) {
				startX = event.pageX;
				startY = event.pageY;
			}
		});

		node.addEventListener('mouseup', function (event) {
			const diffX = Math.abs(event.pageX - startX);
			const diffY = Math.abs(event.pageY - startY);
			// console.log(event.target.id);

			if (diffX < delta && diffY < delta) {
				// Close the dialog
				pushState('', {
					dialog_open: false,
					dialog_id: '',
					dialog_type: ''
				});
			}
		});

		// // Watch for escape key
		// node.addEventListener('keydown', (e) => {
		// 	if (e.key === 'Escape') {
		// 		pushState('', {
		// 			dialog_open: false,
		// 			dialog_id: '',
		// 			dialog_type: ''
		// 		});
		// 	}
		// });
	}

	let copied = false;

	function share() {
		let id = $page.state.dialog_id;

		// /^\d{6}$/m

		if ($page.state.dialog_type === 'bus_trip') {
			// get route ID of trip to monitor
			const trip = $bus_trips.find((t) => t.id === id)!;
			id = `${id}_${trip.route_id}`;
		}

		// dialog type + id
		let url = window.location.origin + `/?dt=${$page.state.dialog_type}&id=${id}`;
		// Only use share api if on mobile and supported
		if (!navigator.share || !/Mobi/i.test(window.navigator.userAgent)) {
			navigator.clipboard.writeText(url);
			copied = true;
			setTimeout(() => {
				copied = false;
			}, 800);
		} else {
			navigator.share({
				title: document.title,
				url
			});
		}
	}

	// Maybe we should pushstate the query params so its easy to copy

	let pin_store: Writable<any>;
	$: item_id = $page.state.dialog_id;

	page.subscribe((p) => {
		switch (p.state.dialog_type) {
			case 'stop':
				pin_store = pinned_stops;
				if (!$stops.some((s) => s.id === p.state.dialog_id)) {
					replaceState('', { ...p.state, dialog_id: 'error' });
				}

				break;
			case 'trip':
				pin_store = pinned_trips;
				if (!$trips.some((t) => t.id === p.state.dialog_id)) {
					replaceState('', { ...p.state, dialog_id: 'error' });
				}
				break;
			case 'route_alert':
				pin_store = pinned_routes;
				if (!all_route_ids.includes(p.state.dialog_id as string)) {
					replaceState('', { ...p.state, dialog_id: 'error' });
				}
				break;
			case 'bus_route_alert':
				pin_store = pinned_bus_routes;
				if (!$bus_routes.some((r) => r.id === (p.state.dialog_id as string))) {
					replaceState('', { ...p.state, dialog_id: 'error' });
				}
				break;
			case 'bus_stop':
				pin_store = pinned_bus_stops;

				const stop_id = parseInt(p.state.dialog_id as string);
				const stop_buses = $bus_stops.filter((s) => s.id === stop_id);

				if (!stop_buses.length) {
					replaceState('', { ...p.state, dialog_id: 'error' });
				} else {
					$monitored_routes = [
						...new Set([...$monitored_routes, ...stop_buses[0].routes.map((r) => r.id)])
					].slice(0, 15);
					// need to replace with int
					replaceState('', { ...p.state, dialog_id: stop_id });
				}

				break;
			// need brackets bc of svelte issue https://github.com/sveltejs/svelte/issues/6706
			case 'bus_trip': {
				pin_store = pinned_bus_trips;

				// first is trip id, the rest are route ids that must be monitored
				const trip_info = (p.state.dialog_id as string).split('_');

				const trip_id = trip_info[0];
				if (trip_info.length > 1) {
					$monitored_routes = [...new Set([...$monitored_routes, ...trip_info.slice(1)])];

					replaceState('', { ...$page.state, dialog_id: trip_id });
				}
				// TODO: figure out a better way to watch for invalid bus trips/stops (some sort of event listener thing or promise)
				// if (!$bus_trips.some((s) => s.id === p.state.dialog_id)) {
				// 	replaceState('', { ...$page.state, dialog_id: 'error' });
				// }

				break;
			}
			default:
				pin_store = pinned_stops;
		}
	});

	let show_previous = false;
</script>

<!-- TODO: figure out transitions -->
<dialog
	id="content-dialog"
	use:manage_dialog
	class="backdrop:bg-black/50 rounded max-h-[90dvh] w-[90vw] max-w-[500px] shadow-lg bg-neutral-800 text-indigo-300"
	bind:this={dialog_el}
>
	<!-- use key to make sure dialog reloads even if only dialog_id has changed -->

	{#key item_id}
		{#if item_id !== 'error'}
			{#if $page.state.dialog_type === 'stop' && typeof item_id === 'string'}
				<StopContent bind:show_previous bind:stop_id={item_id} />
			{:else if $page.state.dialog_type === 'trip' && typeof item_id === 'string'}
				<TripContent bind:show_previous bind:trip_id={item_id} />
			{:else if ($page.state.dialog_type === 'route_alert' || $page.state.dialog_type === 'bus_route_alert') && typeof item_id === 'string'}
				<RouteAlertContent bind:route_id={item_id} bind:route_type={$page.state.dialog_type} />
			{:else if $page.state.dialog_type === 'bus_stop' && typeof item_id === 'number'}
				<BusStopContent bind:stop_id={item_id} />
			{:else if $page.state.dialog_type === 'bus_trip' && typeof item_id === 'string'}
				<BusTripContent bind:trip_id={item_id} />
			{/if}

			<div class="z-40 flex items-center gap-1 justify-between px-2 pt-2">
				<button
					on:click={() => {
						pushState('', {
							dialog_open: false,
							dialog_id: '',
							dialog_type: ''
						});
					}}
					aria-label="Close dialog"
					class="appearance-none h-8 w-8 flex"
				>
					<CircleX />
				</button>

				<div class="flex gap-1 items-center">
					{#if $page.state.dialog_type === 'trip' || $page.state.dialog_type === 'stop'}
						<button
							class={`appearance-none h-8 w-8 flex ${show_previous ? 'text-indigo-600' : ''}`}
							aria-label="Share"
							on:click={() => {
								show_previous = !show_previous;
							}}
						>
							<History class="h-6 w-6" />
						</button>
					{/if}

					{#if !copied}
						<button class="appearance-none h-8 w-8 flex" aria-label="Share" on:click={share}>
							<Share class="h-6 w-6" />
						</button>
					{:else}
						<button
							class="appearance-none flex h-8 w-8 text-green-600"
							aria-label="Link copied to clipboard"
						>
							<ClipboardCheck class="h-6 w-6" />
						</button>
					{/if}
					<!-- TODO: fix so we don't need pb-1 to center -->
					<div class="pb-1">
						<Pin
							store={pin_store}
							item_id={$page.state.dialog_type === 'bus_trip'
								? `${item_id}_${$bus_trips.find((t) => t.id === item_id)?.route_id}`
								: item_id}
						/>
					</div>
				</div>
			</div>
		{:else}
			<h2 class="p-4 items-center text-lg text-red-400 flex gap-2">
				<CircleHelp />
				{$page.state.dialog_type} not found
				<button
					on:click={() => {
						pushState('', {
							dialog_open: true,
							dialog_id: $stops[Math.floor(Math.random() * $stops.length)].id,
							dialog_type: 'stop'
						});
					}}
					aria-label="Random stop"
					class="pl-4 h-10 w-10 text-indigo-700 hover:animate-bounce hover:font-bold"
				>
					<Dices />
				</button>
			</h2>

			<!-- close button -->
			<button
				on:click={() => {
					pushState('', {
						dialog_open: false,
						dialog_id: '',
						dialog_type: ''
					});
				}}
				aria-label="Close dialog"
				class="appearance none h-8 w-8 absolute right-[5px] top-[5px]"
			>
				<CircleX />
			</button>
		{/if}
	{/key}
</dialog>
