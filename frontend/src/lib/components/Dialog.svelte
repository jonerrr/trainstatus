<script lang="ts">
	import { CircleX } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { pushState, preloadData } from '$app/navigation';
	import { all_route_ids } from '$lib/api';
	import { monitored_routes } from '$lib/stores';
	import StopContent from '$lib/components/Stop/Content.svelte';
	import TripContent from '$lib/components/Trip/Content.svelte';
	import RouteAlertContent from '$lib/components/RouteAlert/Content.svelte';
	import BusStopContent from '$lib/components/Stop/BusContent.svelte';
	import BusTripContent from '$lib/components/Trip/BusContent.svelte';

	// detect if user is swiping back and disable close on outside click

	let dialog_el: HTMLDialogElement;

	function manage_dialog(node: HTMLDialogElement) {
		page.subscribe((p) => {
			// console.log(p);
			if (p.state.dialog_open) {
				// prevent close state issues
				// node.close();
				// this prevents auto focusing on close button when opening dialog
				node.inert = true;
				node.showModal();
				node.inert = false;
			} else {
				node.close();
			}
		});

		// This differentiates between a drag and a click so mobile users don't accidentally close the dialog
		// from here https://stackoverflow.com/a/59741870
		const delta = 6;
		let startX: number;
		let startY: number;

		node.addEventListener('mousedown', function (event) {
			startX = event.pageX;
			startY = event.pageY;
		});

		node.addEventListener('mouseup', function (event) {
			const diffX = Math.abs(event.pageX - startX);
			const diffY = Math.abs(event.pageY - startY);

			if (diffX < delta && diffY < delta) {
				if (event.target === node) {
					// Close the dialog
					pushState('', {
						dialog_open: false,
						dialog_id: '',
						dialog_type: ''
					});
				}
			}
		});
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

		const preload_route_ids = $page.url.searchParams.get('pr')?.toUpperCase().split(',');
		if (preload_route_ids) {
			console.log('preloading routes');
			$monitored_routes = [...preload_route_ids, ...$monitored_routes].slice(0, 15);
		}

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
</script>

<!-- TODO: figure out transitions -->
<dialog
	use:manage_dialog
	class="backdrop:bg-black/50 rounded max-h-[85vh] w-[90vw] max-w-[500px] shadow-lg bg-neutral-800 text-indigo-300"
	bind:this={dialog_el}
>
	<!-- use key to make sure dialog reloads even if only dialog_id has changed -->
	{#key $page.state.dialog_id}
		{#if $page.state.dialog_type === 'stop'}
			<StopContent bind:stop_id={$page.state.dialog_id} />
		{:else if $page.state.dialog_type === 'trip'}
			<TripContent bind:trip_id={$page.state.dialog_id} />
		{:else if $page.state.dialog_type === 'route_alert'}
			<RouteAlertContent bind:route_id={$page.state.dialog_id} />
		{:else if $page.state.dialog_type === 'bus_stop'}
			<BusStopContent bind:stop_id={$page.state.dialog_id} />
		{:else if $page.state.dialog_type === 'bus_trip'}
			<BusTripContent bind:trip_id={$page.state.dialog_id} />
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
			class="absolute right-[10px] top-[10px] inline-flex h-8 w-8
                appearance-none items-center justify-center rounded-full"
		>
			<CircleX />
		</button>
	{/key}
</dialog>
