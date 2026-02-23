<script lang="ts">
	import { onMount } from 'svelte';

	import { page } from '$app/state';

	import Icon from '$lib/Icon.svelte';
	import AxisX from '$lib/charts/AxisX.svelte';
	import AxisY from '$lib/charts/AxisY.svelte';
	import Lines from '$lib/charts/Lines.svelte';
	import { type SourceMap, default_sources } from '$lib/resources/index.svelte';
	import { stop_time_context } from '$lib/resources/stop_times.svelte';
	import { trip_context } from '$lib/resources/trips.svelte';
	import { current_time } from '$lib/util.svelte';

	import { Check, ChevronDown, Download, Search, X } from '@lucide/svelte';
	import type { Route, Source } from '@trainstatus/client';
	import { scalePoint, scaleTime } from 'd3-scale';
	import { LayerCake, Svg, flatten } from 'layercake';

	// TODO: maybe somehow include the linecharts in the stop/trip/route modals

	const all_trips = trip_context.get();
	const all_stop_times = stop_time_context.get();

	let routes = $state<SourceMap<Route[]>>({
		mta_subway: [page.data.routes_by_id['mta_subway']['4']],
		// mta_bus: [page.data.routes_by_id['mta_bus']['M15']]
		mta_bus: []
	});

	// 0 = first direction (subway northbound / bus dir 0)
	// 1 = second direction (subway southbound / bus dir 1)
	let direction_index = $state(0);

	// Per-source direction values: subway uses 1 (north) / 3 (south), bus uses 0 / 1
	const source_directions = $derived<SourceMap<number>>({
		mta_subway: direction_index === 0 ? 1 : 3,
		mta_bus: direction_index
	});

	// Flat list of all selected routes (for Lines component, export filename, etc.)
	const selected_routes_flat = $derived(Object.values(routes).flat());

	let stop_points = $state<boolean>(false);
	let current_time_line = $state<boolean>(true);
	// TODO: fix x axis interval causing page to freeze
	let xAxisInterval = $state<number>(15);
	let displayHours = $state<number>(3);

	// Monitor bus routes in the stop_times resource
	$effect(() => {
		const bus_resource = all_stop_times['mta_bus'];
		for (const route of routes['mta_bus']) {
			bus_resource.add_route(route.id);
		}
	});

	const data = $derived.by(() => {
		const route_trips: Array<{
			trip: { id: string; route_id: string; direction: number; [key: string]: unknown };
			points: Array<{ stop_id: string; stop_name: string; time: Date }>;
		}> = [];
		// Track all stops seen across all sources/routes, keyed by stop.id
		const stops_seen = new Map<string, { id: string; name: string; sequence: number }>();

		for (const source of default_sources) {
			const source_routes = routes[source];
			if (!source_routes.length) continue;

			const trips_map = all_trips?.[source]?.value;
			if (!trips_map) continue;

			const stop_times_resource = all_stop_times?.[source];
			const direction = source_directions[source];

			for (const trip of trips_map.values()) {
				if (!source_routes.some((r) => r.id === trip.route_id)) continue;
				if (trip.direction !== direction) continue;

				const trip_st = stop_times_resource?.by_trip_id.get(trip.id);
				if (!trip_st?.length) continue;

				const trip_points: Array<{ stop_id: string; stop_name: string; time: Date }> = [];
				for (const st of trip_st) {
					if (st.arrival.getTime() < current_time.ms) continue;

					const stop = page.data.stops_by_id[source]?.[st.stop_id];
					if (!stop) continue;

					const route_stop = stop.routes.find((r) => r.route_id === trip.route_id);
					const sequence = route_stop?.stop_sequence ?? 0;

					if (!stops_seen.has(stop.id)) {
						stops_seen.set(stop.id, { id: stop.id, name: stop.name, sequence });
					}

					trip_points.push({ stop_id: stop.id, stop_name: stop.name, time: st.arrival });
				}

				if (trip_points.length) {
					route_trips.push({ trip, points: trip_points });
				}
			}
		}

		const yDomain = Array.from(stops_seen.values())
			.sort((a, b) => a.sequence - b.sequence)
			.map((s) => s.name);

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

		// Get SVG dimensions
		const width = svgClone.getAttribute('width');
		const height = svgClone.getAttribute('height');

		const background = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
		background.setAttribute('width', width || '100%');
		background.setAttribute('height', height || '100%');
		background.setAttribute('fill', '#171717');

		// Insert background as first child so it's behind everything else
		svgClone.insertBefore(background, svgClone.firstChild);

		// Add watermark
		const watermark = document.createElementNS('http://www.w3.org/2000/svg', 'text');
		watermark.textContent = 'TrainStat.us';
		watermark.setAttribute('x', '6');
		watermark.setAttribute('y', '10');
		watermark.setAttribute('font-size', '10');
		watermark.setAttribute('fill', '#666666');
		watermark.setAttribute('opacity', '0.6');

		// Add watermark to the SVG
		svgClone.appendChild(watermark);

		// Generate a clean SVG string
		const svgData = new XMLSerializer().serializeToString(svgClone);

		// Create a blob with the SVG data
		const blob = new Blob([svgData], { type: 'image/svg+xml' });

		// Create a download URL
		const url = URL.createObjectURL(blob);

		// Create a filename with all route short names
		const filename = `${selected_routes_flat.map((r) => r.short_name).join('_')}_${direction_index === 0 ? 'northbound' : 'southbound'}_chart.svg`;

		// Create a download link and click it
		const downloadLink = document.createElement('a');
		downloadLink.href = url;
		downloadLink.download = filename;

		// Programmatically click the link to trigger download
		document.body.appendChild(downloadLink);
		downloadLink.click();
		document.body.removeChild(downloadLink);

		// Clean up the URL object
		setTimeout(() => URL.revokeObjectURL(url), 100);
	}

	// Combobox state
	let isComboboxOpen = $state(false);
	let searchQuery = $state('');
	let comboboxRef = $state<HTMLDivElement | null>(null);
	let searchInputRef = $state<HTMLInputElement | null>(null);

	// Sorted flat list of all routes from all sources (subway first, then bus)
	const sortedRoutes = $derived(
		(Object.values(page.data.routes) as Route[][]).flat().sort((a, b) => {
			// Subway routes come first
			if (a.data.source === 'mta_subway' && b.data.source !== 'mta_subway') return -1;
			if (a.data.source !== 'mta_subway' && b.data.source === 'mta_subway') return 1;
			// Then sort by short_name
			return a.short_name.localeCompare(b.short_name);
		})
	);

	// Filter routes based on search query
	const filteredRoutes = $derived(
		sortedRoutes
			.filter((r) => {
				if (!searchQuery) return true;
				const query = searchQuery.toLowerCase();
				const shortName = r.short_name.toLowerCase();
				const longName = r.long_name.toLowerCase();
				const id = r.id.toLowerCase();

				return shortName.includes(query) || longName.includes(query) || id.includes(query);
			})
			.sort((a, b) => {
				// If search query exists and exactly matches a route_id, prioritize it
				if (searchQuery) {
					const query = searchQuery.toLowerCase();
					const aId = a.id.toLowerCase();
					const bId = b.id.toLowerCase();

					if (aId === query && bId !== query) return -1;
					if (bId === query && aId !== query) return 1;
				}
				return 0;
			})
	);

	// Get display name for a route
	function getRouteDisplayName(route: Route) {
		return route.short_name === 'S' ? route.id : route.short_name;
	}

	// Close combobox when clicking outside
	function handleClickOutside(event: MouseEvent) {
		if (comboboxRef && !comboboxRef.contains(event.target as Node)) {
			isComboboxOpen = false;
		}
	}

	// Handle route selection
	function selectRoute(selectedRoute: Route) {
		const source = selectedRoute.data.source as Source;
		if (!routes[source].some((r) => r.id === selectedRoute.id)) {
			routes[source] = [...routes[source], selectedRoute];
		}
		isComboboxOpen = false;
		searchQuery = ''; // Clear search when selection is made
	}

	// Remove a route from selection
	function removeRoute(routeToRemove: Route) {
		// Don't remove if it's the last route
		const total = Object.values(routes).reduce((sum, arr) => sum + arr.length, 0);
		if (total <= 1) return;
		const source = routeToRemove.data.source as Source;
		routes[source] = routes[source].filter((r) => r.id !== routeToRemove.id);
	}

	// Check if route is already selected
	function isRouteSelected(route: Route) {
		const source = route.data.source as Source;
		return routes[source].some((r) => r.id === route.id);
	}

	// Handle combobox keyboard navigation
	function handleComboboxKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			isComboboxOpen = false;
		} else if (event.key === 'ArrowDown') {
			event.preventDefault();
			const firstOption = comboboxRef?.querySelector('[role="option"]') as HTMLElement;
			if (firstOption) firstOption.focus();
		} else if (event.key === 'Enter') {
			event.preventDefault();
			selectRoute(filteredRoutes[0]);
		} else if (event.key === 'Tab' && isComboboxOpen) {
			// Prevent default tab behavior
			event.preventDefault();
			// Focus first option if available
			const firstOption = comboboxRef?.querySelector('[role="option"]') as HTMLElement;
			if (firstOption) firstOption.focus();
		}
	}

	// Handle option keyboard navigation
	function handleOptionKeydown(event: KeyboardEvent, routeOption: Route) {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			selectRoute(routeOption);
		} else if (event.key === 'ArrowDown') {
			event.preventDefault();
			const nextOption = (event.target as HTMLElement).nextElementSibling as HTMLElement;
			if (nextOption) nextOption.focus();
		} else if (event.key === 'ArrowUp') {
			event.preventDefault();
			const prevOption = (event.target as HTMLElement).previousElementSibling as HTMLElement;
			if (prevOption) {
				prevOption.focus();
			} else {
				searchInputRef?.focus();
			}
		}
	}

	// Toggle dropdown and focus search when opening
	function toggleCombobox() {
		isComboboxOpen = !isComboboxOpen;
		if (isComboboxOpen) {
			// Focus the search input when opening
			setTimeout(() => {
				searchInputRef?.focus();
			}, 0);
		}
	}

	// let last_at = current_time.value;

	onMount(() => {
		document.addEventListener('click', handleClickOutside);
		// set current time to 2 hours ago
		// current_time.value = Math.floor((current_time.ms - 4 * 60 * 60 * 1000) / 1000);

		return () => {
			document.removeEventListener('click', handleClickOutside);
			// if (last_at) {
			// 	console.log('setting to old current time', last_at);
			// 	current_time.value = last_at;
			// }
		};
	});

	const xDomain = $derived.by(() => {
		const startTime = new Date(current_time.ms);
		const endTime = new Date(current_time.ms + displayHours * 60 * 60 * 1000);
		// console.log(xDomain);
		// Return domain from current time to current time + displayHours
		return [startTime, endTime];
	});
	// const xRange = $derived(() => {
	// 	const width = svgContainer?.clientWidth || 0;
	// 	return [0, width];
	// });
	// $inspect(xDomain);
</script>

<!-- <svelte:head>
	<title>Charts | Train Status</title>
</svelte:head> -->

<div class="flex h-[calc(100dvh-8rem)] min-h-75 flex-col">
	<div class="pl-3 text-xl font-bold">Charts</div>
	<div
		class="mx-auto flex w-fit flex-wrap gap-6 rounded-md border border-neutral-700/50 bg-neutral-800/70 p-4"
	>
		<!-- Routes Option -->
		<div class="flex min-w-60 flex-col gap-2">
			<div class="font-semibold">Routes</div>
			<div class="relative w-full" bind:this={comboboxRef}>
				<button
					type="button"
					onclick={toggleCombobox}
					aria-haspopup="listbox"
					aria-expanded={isComboboxOpen}
					class="flex w-full cursor-pointer items-center justify-between gap-2 rounded-md border border-neutral-700 bg-neutral-900 px-3 py-2 text-base hover:bg-neutral-800"
					aria-label="Add routes"
				>
					<div class="flex flex-1 flex-wrap items-center gap-2">
						{#if selected_routes_flat.length === 0}
							<span>Select routes</span>
						{:else}
							{#each selected_routes_flat as selectedRoute}
								<div
									class="flex items-center rounded-md border border-neutral-700 bg-neutral-800 px-2 py-1"
								>
									<Icon height={20} width={20} route={selectedRoute} link={false} />
									{#if selected_routes_flat.length > 1}
										<div
											role="button"
											tabindex="0"
											onclick={(e) => {
												e.stopPropagation();
												removeRoute(selectedRoute);
											}}
											onkeydown={(e) => {
												if (e.key === 'Enter' || e.key === ' ') {
													e.preventDefault();
													e.stopPropagation();
													removeRoute(selectedRoute);
												}
											}}
											class="ml-1 cursor-pointer text-neutral-400 hover:text-white"
											aria-label={`Remove ${getRouteDisplayName(selectedRoute)}`}
										>
											<X class="size-3" />
										</div>
									{/if}
								</div>
							{/each}
						{/if}
					</div>
					<ChevronDown class="h-4 w-4 shrink-0" />
				</button>

				<!-- Dropdown with search and options -->
				{#if isComboboxOpen}
					<div
						class="absolute z-10 mt-1 w-64 max-w-64 overflow-hidden rounded-md border border-neutral-700 bg-neutral-900 shadow-lg"
					>
						<div class="border-b border-neutral-700 p-2">
							<div class="relative">
								<div class="pointer-events-none absolute inset-y-0 left-0 flex items-center pl-3">
									<Search class="h-4 w-4 text-neutral-400" />
								</div>
								<input
									type="text"
									bind:this={searchInputRef}
									bind:value={searchQuery}
									onkeydown={handleComboboxKeydown}
									class="block w-full rounded-md border border-neutral-700 bg-neutral-800 py-2 pl-10 pr-3 text-sm placeholder-neutral-400"
									placeholder="Search routes..."
									autocomplete="off"
								/>
							</div>
						</div>

						<!-- Route options -->
						<div id="route-listbox" role="listbox" class="max-h-60 overflow-auto py-1">
							{#if filteredRoutes.length === 0}
								<div class="px-4 py-2 text-neutral-400">No routes found</div>
							{:else}
								{#each filteredRoutes as routeOption}
									<div
										role="option"
										tabindex="0"
										aria-selected={isRouteSelected(routeOption)}
										class="flex cursor-pointer items-center gap-2 px-4 py-2 hover:bg-neutral-800 focus:bg-neutral-800 focus:outline-none {isRouteSelected(
											routeOption
										)
											? 'bg-neutral-700'
											: ''}"
										onclick={() => selectRoute(routeOption)}
										onkeydown={(e) => handleOptionKeydown(e, routeOption)}
									>
										<Icon
											height={32}
											width={32}
											route={routeOption}
											link={false}
											class="mr-3 shrink-0"
										/>
										<span class="grow">{routeOption.long_name}</span>
										<!-- <div class="flex flex-col">
											<span class="text-sm">{getRouteDisplayName(routeOption)}</span>
											<span class="text-neutral-400 text-sm">{routeOption.long_name}</span>
										</div> -->
										{#if isRouteSelected(routeOption)}
											<Check class="mt-1 size-4 shrink-0 text-green-500" />
											<!-- <span class="ml-auto text-green-500">✓</span> -->
										{/if}
									</div>
								{/each}
							{/if}
						</div>
					</div>
				{/if}
			</div>
		</div>

		<!-- Direction Option -->
		<div class="flex min-w-24 flex-col gap-2">
			<div class="font-bold">Direction</div>
			<div class="flex flex-col gap-2 text-neutral-300">
				<div class="flex items-center justify-between gap-2">
					<label for="northbound" class="cursor-pointer">Northbound</label>
					<input
						bind:group={direction_index}
						type="radio"
						id="northbound"
						name="direction"
						value={0}
						class="size-5 cursor-pointer transition-transform hover:scale-110"
					/>
				</div>
				<div class="flex items-center justify-between gap-2">
					<label for="southbound" class="cursor-pointer">Southbound</label>
					<input
						bind:group={direction_index}
						type="radio"
						id="southbound"
						name="direction"
						value={1}
						class="size-5 cursor-pointer transition-transform hover:scale-110"
					/>
				</div>
			</div>
		</div>

		<!-- Stop Points Option -->
		<div class="flex min-w-24 flex-col justify-end gap-2">
			<div class="font-bold">Style</div>
			<div class="flex flex-col gap-2 text-neutral-300">
				<div class="flex w-full items-center justify-between gap-2">
					<label for="stop_points" class="cursor-pointer">Stop Points</label>
					<input
						id="stop_points"
						type="checkbox"
						name="stop_points"
						bind:checked={stop_points}
						class="size-5 cursor-pointer transition-transform hover:scale-110"
					/>
				</div>
				<div class="flex w-full justify-between gap-2">
					<label for="time_line" class="cursor-pointer">Time Line</label>
					<input
						id="time_line"
						type="checkbox"
						name="time_line"
						bind:checked={current_time_line}
						class="size-5 cursor-pointer transition-transform hover:scale-110"
					/>
				</div>
			</div>
		</div>

		<!-- X-Axis Interval Slider -->
		<div class="flex min-w-40 flex-col gap-2">
			<div class="font-semibold">Time Interval</div>
			<div class="flex flex-col gap-1">
				<div class="flex items-center gap-2">
					<input
						type="range"
						min="5"
						max="30"
						step="5"
						bind:value={xAxisInterval}
						class="w-full cursor-pointer"
					/>
					<span class="w-8 text-right">{xAxisInterval}m</span>
				</div>
			</div>
		</div>

		<!-- Display Hours Slider -->
		<div class="flex min-w-40 flex-col gap-2">
			<div class="font-semibold">Number of Hours</div>
			<div class="flex flex-col gap-1">
				<div class="flex items-center gap-2">
					<input
						type="range"
						min="1"
						max="4"
						step="1"
						bind:value={displayHours}
						class="w-full cursor-pointer"
					/>
					<span class="w-8 text-right">{displayHours}h</span>
				</div>
			</div>
		</div>

		<!-- Export Button -->
		<div class="flex min-w-30 flex-col justify-center">
			<button
				onclick={export_as_svg}
				class="flex h-fit items-center justify-center gap-2 rounded bg-neutral-900 px-4 py-2 transition-colors hover:bg-neutral-800"
				aria-label="Export chart as SVG"
			>
				<Download class="size-5" />
				Export SVG
			</button>
		</div>
	</div>

	<!-- Chart container with proper overflow handling -->
	<div class="relative flex-1 overflow-hidden">
		{#if data.route_trips.length && data.yDomain.length}
			<!-- Scrollable container with both scroll directions -->
			<div class="absolute inset-0 overflow-auto">
				<!-- Chart with minimum dimensions but able to shrink -->
				<div bind:this={svgContainer} class="h-full min-h-125 w-full min-w-325">
					<!-- TODO: fix lines where the arrival times are identical (because mta is bad) from creating invalid lines (crosses same Y twice) -->
					<LayerCake
						debug={false}
						ssr
						padding={{ top: 20, right: 10, left: 160, bottom: 30 }}
						x="time"
						y="stop_name"
						{xDomain}
						yDomain={data.yDomain}
						yScale={scalePoint().padding(0)}
						xScale={scaleTime()}
						data={data.route_trips}
						flatData={flatten(data.route_trips, 'points')}
					>
						<Svg>
							<AxisX {current_time_line} interval={xAxisInterval} />
							<AxisY />
							<Lines routes={selected_routes_flat} bind:stop_points />
						</Svg>
					</LayerCake>
				</div>
			</div>
		{:else}
			<div class="flex h-full items-center justify-center text-neutral-400">No data available</div>
		{/if}
	</div>
</div>
