<script lang="ts">
	import { CircleX, Share, ClipboardCheck, CircleHelp, Dices } from 'lucide-svelte';
	import type { Writable } from 'svelte/store';
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
		pinned_stops,
		pinned_trips
	} from '$lib/stores';
	import StopContent from '$lib/components/Stop/Content.svelte';
	import TripContent from '$lib/components/Trip/Content.svelte';
	import RouteAlertContent from '$lib/components/RouteAlert/Content.svelte';
	import BusStopContent from '$lib/components/Stop/BusContent.svelte';
	import BusTripContent from '$lib/components/Trip/BusContent.svelte';
	import Pin from '$lib/components/Pin.svelte';

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
				document.title = 'Trainstat.us | Home';
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
	}

	let copied = false;

	// for bus trips, we need to also specify the route id to monitor, otherwise the trip won't show up
	let preload_bus_route: string;

	function share() {
		let title = '';
		switch ($page.state.dialog_type) {
			case 'stop':
				// param = 's';
				title = 'View Stop';
				break;
			case 'trip':
				// param = 't';
				title = 'View Trip';
				break;
			case 'route_alert':
				// param = 'r';
				title = 'View Route Alert';
				break;
			case 'bus_stop':
				// param = 'bs';
				title = 'View Bus Stop';
				// don't need to preload bus stops bc it is checked in other component
				// const stop = $bus_stops.find((s) => s.id === $page.state.dialog_id)!;
				// preload_bus_route = stop.routes.map((r) => r.id).join(',');
				break;
			case 'bus_trip':
				// param = 'bt';
				title = 'View Bus Trip';
				const trip = $bus_trips.find((t) => t.id === $page.state.dialog_id)!;
				preload_bus_route = trip.route_id;
				break;
		}

		// dialog type + id
		let url =
			window.location.origin + `/?dt=${$page.state.dialog_type}&id=${$page.state.dialog_id}`;
		if (preload_bus_route) {
			url += `&pr=${preload_bus_route}`;
		}

		if (!navigator.share) {
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
	}

	// Maybe we should pushstate the query params so its easy to copy

	let pin_store: Writable<any>;
	$: item_id = $page.state.dialog_id;

	page.subscribe((p) => {
		switch (p.state.dialog_type) {
			case 'stop':
				pin_store = pinned_stops;
				if (!$stops.some((s) => s.id === p.state.dialog_id)) {
					replaceState('', { ...$page.state, dialog_id: 'error' });
				}

				break;
			case 'trip':
				pin_store = pinned_trips;
				if (!$trips.some((t) => t.id === p.state.dialog_id)) {
					replaceState('', { ...$page.state, dialog_id: 'error' });
				}
				break;
			case 'route_alert':
				pin_store = pinned_routes;
				if (!all_route_ids.includes(p.state.dialog_id as string)) {
					replaceState('', { ...$page.state, dialog_id: 'error' });
				}
				break;
			case 'bus_stop':
				pin_store = pinned_bus_stops;
				if (!$bus_stops.some((s) => s.id === p.state.dialog_id)) {
					replaceState('', { ...$page.state, dialog_id: 'error' });
				}

				break;
			// need brackets bc of svelte issue https://github.com/sveltejs/svelte/issues/6706
			case 'bus_trip': {
				pin_store = pinned_bus_trips;
				// TODO: implement bus trips
				// need to preload the route for bus trips
				const trip = $bus_trips.find((t) => t.id === p.state.dialog_id)!;
				item_id = `${trip.route_id}_${p.state.dialog_id}`;
				// preload_bus_route = trip.route_id;
				break;
			}
			default:
				pin_store = pinned_stops;
		}
	});
	// used to set the max width of the content titles
	let actions_width: number;
</script>

<!-- TODO: figure out transitions -->
<dialog
	id="content-dialog"
	use:manage_dialog
	class="backdrop:bg-black/50 rounded max-h-[85dvh] w-[90vw] max-w-[500px] shadow-lg bg-neutral-800 text-indigo-300"
	bind:this={dialog_el}
>
	<!-- use key to make sure dialog reloads even if only dialog_id has changed -->

	{#key item_id}
		{#if item_id !== 'error'}
			{#if $page.state.dialog_type === 'stop' && typeof item_id === 'string'}
				<StopContent bind:actions_width bind:stop_id={item_id} />
			{:else if $page.state.dialog_type === 'trip' && typeof item_id === 'string'}
				<TripContent bind:actions_width bind:trip_id={item_id} />
			{:else if $page.state.dialog_type === 'route_alert' && typeof item_id === 'string'}
				<RouteAlertContent bind:route_id={item_id} />
			{:else if $page.state.dialog_type === 'bus_stop' && typeof item_id === 'number'}
				<BusStopContent bind:stop_id={item_id} />
			{:else if $page.state.dialog_type === 'bus_trip' && typeof item_id === 'string'}
				<BusTripContent bind:actions_width bind:trip_id={item_id} />
			{/if}

			<div
				bind:offsetWidth={actions_width}
				class="z-40 absolute right-[5px] top-[10px] inline-flex gap-1 items-center"
			>
				<Pin store={pin_store} {item_id} />

				{#if !copied}
					<button class="appearance-none inline-flex h-8 w-8" aria-label="Share" on:click={share}>
						<Share class="h-6 w-6" />
					</button>
				{:else}
					<button
						class="appearance-none inline-flex h-8 w-8 text-green-600"
						aria-label="Link copied to clipboard"
					>
						<ClipboardCheck class="h-6 w-6" />
					</button>
				{/if}

				<button
					on:click={() => {
						pushState('', {
							dialog_open: false,
							dialog_id: '',
							dialog_type: ''
						});
					}}
					aria-label="Close dialog"
					class="appearance-none inline-flex h-8 w-8"
				>
					<CircleX />
				</button>
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
				class="appearance none inline-flex h-8 w-8 absolute right-[5px] top-[5px]"
			>
				<CircleX />
			</button>
		{/if}
	{/key}
</dialog>
