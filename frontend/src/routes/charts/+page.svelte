<script lang="ts">
	import { LayerCake, Svg, flatten } from 'layercake';
	import { Train, BusFront, Download } from 'lucide-svelte';
	import { scaleTime, scalePoint } from 'd3-scale';
	import { page } from '$app/state';
	import { TripDirection, trips } from '$lib/trips.svelte';
	import { monitored_bus_routes, stop_times } from '$lib/stop_times.svelte';
	import { type Route } from '$lib/static';
	import AxisX from './AxisX.svelte';
	import AxisY from './AxisY.svelte';
	import Lines from './Lines.svelte';

	let direction = $state<TripDirection>(TripDirection.North);
	let route = $state<Route>(page.data.routes['3']);

	$effect(() => {
		if (route.route_type === 'bus') {
			monitored_bus_routes.add(route.id);
		}
	});

	// TODO: add main route stops to y domain no matter what
	const data = $derived.by(() => {
		// if (!trips.trips || !route) return;

		const route_trips = [];
		// keep track of active stops so we know what the y domain should be
		const route_stops = new Set<{ id: number; name: string; sequence: number }>();

		for (const trip of trips.trips.values()) {
			if (trip.route_id !== route.id || trip.direction !== direction) continue;
			const trip_st = stop_times.by_trip_id[trip.id];
			if (!trip_st) continue;
			const trip_points = [];
			for (const st of trip_st) {
				const stop = page.data.stops[st.stop_id];

				const stop_sequence = stop.routes.find((r) => r.id === trip.route_id)?.stop_sequence;
				if (!stop_sequence) {
					console.log('stop_sequence not found, skipping', stop.name, trip.route_id);
					continue;
				}

				route_stops.add({ id: stop.id, name: stop.name, sequence: stop_sequence });
				trip_points.push({
					// trip: trip,
					// route_id: trip.route_id,
					// stop_id: st.stop_id,
					stop_name: stop.name,
					// sequence: stop_sequence,
					time: st.arrival
				});
			}

			route_trips.push({ trip, points: trip_points });
		}

		const yDomain = Array.from(route_stops)
			.sort((a, b) => a.sequence - b.sequence)
			.map((stop) => stop.name);

		return { route_trips, yDomain };
	});

	// Create a reference to the SVG element
	let svgContainer = $state<HTMLDivElement>();

	function export_as_svg() {
		if (!svgContainer) return;

		// Find the SVG element inside the container
		const svgElement = svgContainer.querySelector('svg');
		if (!svgElement) return;

		// Clone the SVG to avoid modifying the original
		const svgClone = svgElement.cloneNode(true) as SVGElement;

		// Add required attributes for standalone SVG
		svgClone.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
		svgClone.setAttribute('version', '1.1');

		// Generate a clean SVG string
		const svgData = new XMLSerializer().serializeToString(svgClone);

		// Create a blob with the SVG data
		const blob = new Blob([svgData], { type: 'image/svg+xml' });

		// Create a download URL
		const url = URL.createObjectURL(blob);

		// Create a download link and click it
		const downloadLink = document.createElement('a');
		downloadLink.href = url;
		downloadLink.download = `${route.short_name}_${direction === TripDirection.North ? 'northbound' : 'southbound'}_chart.svg`;

		// Programmatically click the link to trigger download
		document.body.appendChild(downloadLink);
		downloadLink.click();
		document.body.removeChild(downloadLink);

		// Clean up the URL object
		setTimeout(() => URL.revokeObjectURL(url), 100);
	}
</script>

<svelte:head>
	<title>Charts | TrainStat.us</title>
</svelte:head>

<div class="flex flex-col gap-1 rounded">
	<div class="text-xl font-bold px-2">Charts</div>
	<div class="flex gap-4 bg-neutral-950 p-2 w-full items-start justify-between">
		<div class="flex gap-4">
			<div class="grid grid-rows-3 gap-2">
				<div class="font-semibold">Direction</div>
				<div class="flex items-center">
					<input
						bind:group={direction}
						type="radio"
						id="northbound"
						name="direction"
						value={TripDirection.North}
						class="mr-2"
					/>
					<label for="northbound">Northbound</label>
				</div>
				<div class="flex items-center">
					<input
						bind:group={direction}
						type="radio"
						id="southbound"
						name="direction"
						value={TripDirection.South}
						class="mr-2"
					/>
					<label for="southbound">Southbound</label>
				</div>
			</div>
			<div class="flex flex-col gap-2">
				<div class="font-semibold">Route</div>
				<div class="relative w-64">
					<div class="pointer-events-none absolute inset-y-0 left-0 flex items-center pl-3">
						{#if route.route_type === 'train'}
							<Train class="h-5 w-5 text-neutral-400" />
						{:else}
							<BusFront class="h-5 w-5 text-neutral-400" />
						{/if}
					</div>

					<select
						bind:value={route}
						class="block w-full rounded-md border border-neutral-700 bg-neutral-900 py-2 pl-10 pr-3 text-base focus:border-blue-500 focus:outline-none focus:ring-blue-500"
					>
						<option value="" disabled selected>Select a route</option>
						{#each Object.values(page.data.routes).sort((a, b) => {
							// Train routes come first
							if (a.route_type === 'train' && b.route_type !== 'train') return -1;
							if (a.route_type !== 'train' && b.route_type === 'train') return 1;
							// Then sort by short_name
							return a.short_name.localeCompare(b.short_name);
						}) as route}
							<option value={route}>
								{route.short_name}
							</option>
						{/each}
					</select>
				</div>
			</div>
		</div>

		<button
			onclick={export_as_svg}
			class="flex items-center gap-2 bg-neutral-800 hover:bg-neutral-700 text-white py-2 px-4 rounded"
			aria-label="Export chart as SVG"
		>
			<Download size={16} />
			Export SVG
		</button>
	</div>
	{#if data.route_trips.length && data.yDomain.length}
		<div bind:this={svgContainer} class="w-[100dvw] h-[700px]">
			<LayerCake
				ssr
				padding={{ top: 20, right: 10, left: 160, bottom: 30 }}
				x="time"
				y="stop_name"
				yDomain={data.yDomain}
				yScale={scalePoint().padding(0.5)}
				xScale={scaleTime()}
				data={data.route_trips}
				flatData={flatten(data.route_trips, 'points')}
			>
				<Svg>
					<AxisX />
					<AxisY />
					<Lines stroke="#{route.color}" />
				</Svg>
			</LayerCake>
		</div>
	{:else}
		<div class="flex items-center justify-center h-full text-neutral-400">No data available</div>
	{/if}
</div>
