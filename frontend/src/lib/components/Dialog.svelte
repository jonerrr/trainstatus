<script lang="ts">
	import { CircleX } from 'lucide-svelte';
	import { createDialog, melt, createSync } from '@melt-ui/svelte';
	import { onMount } from 'svelte';
	import { page } from '$app/stores';
	import { pushState, preloadData } from '$app/navigation';
	import { flyAndScale } from '$lib/utils';
	import StopContent from '$lib/components/Stop/Content.svelte';
	import TripContent from '$lib/components/Trip/Content.svelte';
	import RouteAlertContent from '$lib/components/RouteAlert/Content.svelte';

	// detect if user is swiping back and disable close on outside click

	const {
		elements: { trigger, overlay, content, title, description, close, portalled },
		states: { open }
	} = createDialog({
		forceVisible: true,
		closeOnOutsideClick: false
	});

	const sync = createSync({ open });
	$: sync.open($page.state.dialog_open, ($open) => {
		console.log('dialog opened', $open, $page.state);
		open.set($open);
	});

	// Check if user is trying to open a dialog from the URL
	// Maybe we should pushstate the query params so its easy to copy
	onMount(async () => {
		const open_stop_id = $page.url.searchParams.get('s');
		const open_route_id = $page.url.searchParams.get('r');
		const open_trip_id = $page.url.searchParams.get('t');

		if (open_stop_id) {
			// Make sure data is loaded in before opening dialog otherwise we get an error
			await preloadData('/');
			pushState('', { dialog_open: true, dialog_id: open_stop_id, dialog_type: 'stop' });
		} else if (open_route_id) {
			await preloadData('/');
			pushState('', { dialog_open: true, dialog_id: open_route_id, dialog_type: 'route_alert' });
		} else if (open_trip_id) {
			await preloadData('/');
			pushState('', { dialog_open: true, dialog_id: open_trip_id, dialog_type: 'trip' });
		}
	});
</script>

<!-- TODO: figure out a way to prevent swiping from closing dialog on mobile -->
{#if $open}
	<div use:melt={$portalled}>
		<div use:melt={$overlay} class="fixed inset-0 z-50 bg-black/50" />
		<div
			class="fixed left-1/2 top-1/2 z-50 max-h-[85vh] w-[90vw]
            max-w-[450px] -translate-x-1/2 -translate-y-1/2 rounded-md
            p-6 shadow-lg bg-neutral-800 text-indigo-300"
			transition:flyAndScale={{
				duration: 150,
				y: 8,
				start: 0.96
			}}
			use:melt={$content}
		>
			{#if $page.state.dialog_type === 'stop'}
				<StopContent stop_id={$page.state.dialog_id} />
			{:else if $page.state.dialog_type === 'trip'}
				<TripContent trip_id={$page.state.dialog_id} />
			{:else if $page.state.dialog_type === 'route_alert'}
				<RouteAlertContent route_id={$page.state.dialog_id} />
			{/if}

			<button
				use:melt={$close}
				aria-label="Close"
				class="absolute right-[10px] top-[10px] inline-flex h-8 w-8
                appearance-none items-center justify-center rounded-full"
			>
				<CircleX />
			</button>
		</div>
	</div>
{/if}
