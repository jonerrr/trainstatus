<script lang="ts">
	import { CircleX } from 'lucide-svelte';
	import { createDialog, melt, createSync } from '@melt-ui/svelte';
	import { page } from '$app/stores';
	import { flyAndScale } from '$lib/utils';
	import StopContent from '$lib/components/Stop/Content.svelte';
	import TripContent from '$lib/components/Trip/Content.svelte';
	import RouteAlertContent from '$lib/components/RouteAlert/Content.svelte';

	const {
		elements: { trigger, overlay, content, title, description, close, portalled },
		states: { open }
	} = createDialog({
		forceVisible: true
	});

	const sync = createSync({ open });
	$: sync.open($page.state.dialog_open, ($open) => {
		console.log('dialog opened', $open, $page.state);
		open.set($open);
	});
</script>

{#if $open}
	<div class="" use:melt={$portalled}>
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
				class="absolute right-[10px] top-[10px] inline-flex h-6 w-6
                appearance-none items-center justify-center rounded-full text-magnum-800
                hover:bg-magnum-100 focus:shadow-magnum-400"
			>
				<CircleX />
			</button>
		</div>
	</div>
{/if}
