<script lang="ts">
	import { CircleX, Share, ClipboardCheck } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import type { Writable } from 'svelte/store';
	import { page } from '$app/stores';
	import { pushState, preloadData } from '$app/navigation';
	import { all_route_ids } from '$lib/api';
	import {
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
		let param = '';
		let title = '';
		switch ($page.state.dialog_type) {
			case 'stop':
				param = 's';
				title = 'View Stop';
				break;
			case 'trip':
				param = 't';
				title = 'View Trip';
				break;
			case 'route_alert':
				param = 'r';
				title = 'View Route Alert';
				break;
			case 'bus_stop':
				param = 'bs';
				title = 'View Bus Stop';
				// don't need to preload bus stops bc it is checked in other component
				// const stop = $bus_stops.find((s) => s.id === $page.state.dialog_id)!;
				// preload_bus_route = stop.routes.map((r) => r.id).join(',');
				break;
			case 'bus_trip':
				param = 'bt';
				title = 'View Bus Trip';
				const trip = $bus_trips.find((t) => t.id === $page.state.dialog_id)!;
				preload_bus_route = trip.route_id;
				break;
		}

		let url = window.location.origin + `/?${param}=${$page.state.dialog_id}`;
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

	// Check if user is trying to open a dialog from the URL
	// Maybe we should pushstate the query params so its easy to copy
	onMount(async () => {
		// stop and routes should be uppercase but trip ids should be lowercase because they are uuids
		const open_stop_id = $page.url.searchParams.get('s')?.toUpperCase();
		const open_route_id = $page.url.searchParams.get('r')?.toUpperCase();
		const open_trip_id = $page.url.searchParams.get('t')?.toLowerCase();

		const open_bus_stop_id = $page.url.searchParams.get('bs');
		const open_bus_trip_id = $page.url.searchParams.get('bt')?.toLowerCase();

		if (open_stop_id) {
			// Make sure data is loaded in before opening dialog otherwise we get an error
			await preloadData('/');
			pushState('', {
				dialog_open: true,
				dialog_id: open_stop_id,
				dialog_type: 'stop'
			});
		} else if (open_route_id) {
			if (all_route_ids.includes(open_route_id)) {
				await preloadData('/');
				pushState('', {
					dialog_open: true,
					dialog_id: open_route_id,
					dialog_type: 'route_alert'
				});
			} else {
				console.error('invalid route id');
			}
		} else if (open_trip_id) {
			await preloadData('/');
			pushState('', {
				dialog_open: true,
				dialog_id: open_trip_id,
				dialog_type: 'trip'
			});
		} else if (open_bus_stop_id) {
			await preloadData('/');

			pushState('', {
				dialog_open: true,
				dialog_id: parseInt(open_bus_stop_id),
				dialog_type: 'bus_stop'
			});
		} else if (open_bus_trip_id) {
			await preloadData('/');

			pushState('', {
				dialog_open: true,
				dialog_id: open_bus_trip_id,
				dialog_type: 'bus_trip'
			});
		}
	});

	let pin_store: Writable<any>;
	$: pin_id = $page.state.dialog_id;

	$: switch ($page.state.dialog_type) {
		case 'stop':
			pin_store = pinned_stops;
			break;
		case 'trip':
			pin_store = pinned_trips;
			break;
		case 'route_alert':
			pin_store = pinned_routes;
			break;
		case 'bus_stop':
			pin_store = pinned_bus_stops;
			break;
		// need brackets bc of svelte issue https://github.com/sveltejs/svelte/issues/6706
		case 'bus_trip': {
			pin_store = pinned_bus_trips;
			// TODO: fix this
			// need to preload the route for bus trips
			const trip = $bus_trips.find((t) => t.id === $page.state.dialog_id)!;
			pin_id = `${trip.route_id}_${$page.state.dialog_id}`;
			// preload_bus_route = trip.route_id;
			break;
		}
		default:
			pin_store = pinned_stops;
	}

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
	{#key $page.state.dialog_id}
		{#if $page.state.dialog_type === 'stop'}
			<StopContent bind:actions_width bind:stop_id={$page.state.dialog_id} />
		{:else if $page.state.dialog_type === 'trip'}
			<TripContent bind:actions_width bind:trip_id={$page.state.dialog_id} />
		{:else if $page.state.dialog_type === 'route_alert'}
			<RouteAlertContent bind:route_id={$page.state.dialog_id} />
		{:else if $page.state.dialog_type === 'bus_stop'}
			<BusStopContent bind:stop_id={$page.state.dialog_id} />
		{:else if $page.state.dialog_type === 'bus_trip'}
			<BusTripContent bind:actions_width bind:trip_id={$page.state.dialog_id} />
		{/if}

		<div
			bind:offsetWidth={actions_width}
			class="z-40 absolute right-[5px] top-[10px] inline-flex gap-1 items-center"
		>
			<Pin store={pin_store} item_id={pin_id} />

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
	{/key}
</dialog>
