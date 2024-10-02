<script lang="ts">
	import type { Stop } from '$lib/static';
	import { type PersistedRune } from '$lib/util.svelte';
	import Button from '$lib/Button.svelte';

	interface ButtonProps {
		stop: Stop<'train' | 'bus'>;
		pin_rune: PersistedRune<number[]>;
	}

	let { stop, pin_rune }: ButtonProps = $props();
</script>

{#snippet arrivals(headsign: string)}
	<div class="text-xs text-neutral-200 text-wrap text-left pb-1">
		{headsign}
	</div>
	<!-- <div class="flex flex-col gap-1"> -->
	<!-- {#each base_routes as route_id}
					<Arrivals {route_id} {stop} direction={Direction.North} />
				{/each}
				{#each other_route_ids as route_id}
					<Arrivals {route_id} {stop} direction={Direction.North} base_route={false} />
				{/each} -->
	<!-- </div> -->
{/snippet}
<Button
	state={{
		dialog_id: stop.id,
		dialog_type: 'stop',
		dialog_open: true,
		data: stop
	}}
	{pin_rune}
>
	{#if stop.type === 'train'}
		<div class="w-[25%] grow-0 text-neutral-100">
			{stop.name}
		</div>

		<div class="flex flex-col w-[30%] mt-auto">
			<!-- <div class="flex flex-col gap-1"> -->
			<!-- <p>asdads</p> -->
			{@render arrivals(stop.north_headsign)}
			{@render arrivals(stop.south_headsign)}
			<!-- </div> -->
		</div>

		<div class="flex flex-col w-[30%] mt-auto">
			<div class="text-xs text-indigo-200 text-wrap text-left pb-1">
				<!-- {stop.south_headsign} -->
			</div>
			<div class="flex flex-col gap-1">
				<!-- {#each base_routes as route_id}
						<Arrivals {route_id} {stop} direction={Direction.South} />
					{/each}
					{#each other_route_ids as route_id}
						<Arrivals {route_id} {stop} direction={Direction.South} base_route={false} />
					{/each} -->
			</div>
		</div>
	{:else}
		<!-- TODO: make spacing consistent (use grid maybe idk) -->
		<div class="flex flex-col text-left text-xs">
			<div class="flex gap-2">
				<div>
					<!-- {#if stop.direction === 'SW'}
					<ArrowDownLeft size={16} />
				{:else if stop.direction === 'S'}
					<ArrowDown size={16} />
				{:else if stop.direction === 'SE'}
					<ArrowDownRight size={16} />
				{:else if stop.direction === 'E'}
					<ArrowRight size={16} />
				{:else if stop.direction === 'W'}
					<ArrowLeft size={16} />
				{:else if stop.direction === 'NW'}
					<ArrowUpLeft size={16} />
				{:else if stop.direction === 'NE'}
					<ArrowUpRight size={16} />
				{:else if stop.direction === 'N'}
					<ArrowUp size={16} />
				{/if} -->
				</div>
				<div class="font-bold">
					{stop.name}
				</div>
				<div class="text-neutral-300 font-bold">
					#{stop.id}
				</div>
			</div>

			<div class="flex flex-col">
				<!-- {#each stop_routes as route}
					<div class="flex gap-2 items-center text-xs text-wrap text-left rounded p-1">
						<BusIcon {route} />
						<div class="text-neutral-100 max-w-[60%]">
							{stop.routes.find((r) => r.id === route.id)?.headsign}
						</div>

						<div class="">
							<BusArrivals route_id={route.id} stop_id={stop.id} />
						</div>
					</div>
				{/each} -->
			</div>
		</div>
	{/if}
</Button>
