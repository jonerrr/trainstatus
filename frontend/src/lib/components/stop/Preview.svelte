<script lang="ts">
	import { melt, createTabs } from '@melt-ui/svelte';
	import { cubicInOut } from 'svelte/easing';
	import { crossfade } from 'svelte/transition';
	import { Dialog } from '$lib/components/Dialog';
	import { Direction, type Stop, type Trip } from '$lib/api';
	import { pinned_stops, stops } from '$lib/stores';
	import Pin from '$lib/components/Pin.svelte';
	import Eta from '$lib/components/stop/Eta.svelte';
	import Trips from '$lib/components/stop/Trips.svelte';
	import Icon from '$lib/components/Icon.svelte';

	export let stop: Stop;
	// default to empty array to prevent undefined errors
	export let trips: Trip[] = [];
	// console.log(trips);
	// console.log(stop);

	$: trips_with_eta = trips
		.filter((trip) => trip.stop_times.some((st) => st.stop_id === stop.id))
		.map((trip) => {
			const stop_time = trip.stop_times.find((time) => time.stop_id === stop.id)!;

			const arrival = new Date(stop_time.arrival).getTime();
			const now = new Date().getTime();
			const eta = (arrival - now) / 1000 / 60;

			// console.log(trip.route_id, eta, stop_time.arrival);
			console.log('mapping');
			// TODO: fix this
			if (eta < 0) {
				console.log(
					'how the fuck is this trip in the past:',
					trip.route_id,
					eta,
					stop_time.arrival
				);
			}

			return {
				...trip,
				eta
			};
		})
		.sort((a, b) => a.eta - b.eta);

	$: northbound = trips_with_eta.filter((trip) => trip.direction === Direction.North);
	$: southbound = trips_with_eta.filter((trip) => trip.direction === Direction.South);

	// const stop_routes = stop.routes.flatMap((route) => route.route_id);
	const {
		elements: { root, list, content, trigger },
		states: { value }
	} = createTabs({
		defaultValue: 'northbound'
	});

	// TODO: replace title with more specific destination (maybe last stop boroughs or headsign)
	const triggers = [
		{ id: 'northbound', title: 'Northbound' },
		{ id: 'southbound', title: 'Southbound' }
	];

	const [send, receive] = crossfade({
		duration: 250,
		easing: cubicInOut
	});
</script>

<Dialog.Trigger name={stop.id}>
	<div class="w-24 grow-0 font-semibold text-indigo-300">
		{stop.name}
	</div>

	<!-- northbound trips -->
	<div class="flex flex-col items-center">
		<div class="text-xs">
			{stop.north_headsign}
		</div>
		<div class="flex grow-0 w-24">
			<Eta routes={stop.routes} bind:trips={northbound} />
		</div>
	</div>

	<!-- southbound trips -->
	<div class="flex flex-col items-center">
		<div class="text-xs">
			{stop.south_headsign}
		</div>
		<div class="flex grow-0 w-24">
			<Eta routes={stop.routes} bind:trips={southbound} />
		</div>
	</div>

	<div>
		<Pin item_id={stop.id} store={pinned_stops} />
	</div>
</Dialog.Trigger>

<Dialog.Content name={stop.id} let:title let:description let:close>
	<h2 class="font-bold flex items-center gap-2 text-indigo-300" use:melt={title}>
		{stop.name}

		<div class="flex gap-1">
			{#each stop.routes as route (route.id)}
				<Icon name={route.id} />
			{/each}
		</div>
	</h2>
	<div use:melt={description}>
		<div
			use:melt={$root}
			class="flex max-w-[25rem] flex-col overflow-hidden rounded-xl shadow-lg data-[orientation=vertical]:flex-row bg-neutral-600 text-indigo-200"
		>
			<div
				use:melt={$list}
				class="flex shrink-0 overflow-x-auto text-indigo-100
		  data-[orientation=vertical]:flex-col data-[orientation=vertical]:border-r"
				aria-label="Trip information"
			>
				{#each triggers as triggerItem}
					<button use:melt={$trigger(triggerItem.id)} class="trigger relative">
						{triggerItem.title}
						{#if $value === triggerItem.id}
							<div
								in:send={{ key: 'trigger' }}
								out:receive={{ key: 'trigger' }}
								class="absolute bottom-1 left-1/2 h-1 w-6 -translate-x-1/2 rounded-full bg-indigo-400"
							/>
						{/if}
					</button>
				{/each}
			</div>
			<div use:melt={$content('northbound')} class="grow bg-neutral-600 p-2">
				<Trips bind:trips={northbound} />
			</div>
			<div use:melt={$content('southbound')} class="grow bg-neutral-600 p-2">
				<Trips bind:trips={southbound} />
			</div>
		</div>

		<div class="flex text-indigo-200">
			<button class="btn mt-2 ml-auto" use:melt={close}>Close</button>
		</div>
	</div>
</Dialog.Content>

<style lang="postcss">
	.trigger {
		display: flex;
		align-items: center;
		justify-content: center;

		cursor: default;
		user-select: none;

		border-radius: 0;

		/* color: theme(colors.neutral.900); */
		font-weight: 500;
		line-height: 1;

		flex: 1;
		height: theme(spacing.12);
		padding-inline: theme(spacing.2);

		&:focus {
			position: relative;
		}

		&:focus-visible {
			@apply z-10 ring-2;
		}

		&[data-state='active'] {
			@apply focus:relative;
			/* background-color: white; */
			color: theme('colors.indigo.200');
		}
	}

	/* .save {
		display: inline-flex;
		height: theme(spacing.8);
		cursor: default;
		align-items: center;
		justify-content: center;
		border-radius: theme(borderRadius.md);
		background-color: theme(colors.zinc.200);
		padding-inline: theme(spacing.4);
		line-height: 1;
		font-weight: theme(fontWeight.semibold);
		color: theme(colors.zinc.900);
		@apply transition;

		&:hover {
			opacity: 0.75;
		}

		&:focus {
			@apply !ring-green-600;
		}
	} */
</style>
