<script lang="ts">
	import { LayerCake, Svg, flatten } from 'layercake';
	import { Download, Search, ChevronDown, X, Check } from 'lucide-svelte';
	import { scaleTime, scalePoint } from 'd3-scale';
	import { page } from '$app/state';
	import { TripDirection, trips } from '$lib/trips.svelte';
	import { monitored_bus_routes, stop_times } from '$lib/stop_times.svelte';
	import { type Route } from '$lib/static';
	import AxisX from './AxisX.svelte';
	import AxisY from './AxisY.svelte';
	import Lines from './Lines.svelte';
	import { current_time } from '$lib/util.svelte';
	import { onMount } from 'svelte';
	import Icon from '$lib/Icon.svelte';

	let routes = $state<Route[]>([page.data.routes['3']]);
	let direction = $state<TripDirection>(TripDirection.North);
	let stop_points = $state<boolean>(false);
	let current_time_line = $state<boolean>(true);
	let xAxisInterval = $state<number>(15);
	let displayHours = $state<number>(3);

	$effect(() => {
		for (const r of routes) {
			if (r.route_type === 'bus') {
				monitored_bus_routes.add(r.id);
			}
		}
	});

	// interface RouteStopData {
	// 	stop_id: number;
	// 	stop_name: string;
	// 	// sequence of this stop in stop times for each of the trips
	// 	sequences: number[];
	// }

	// TODO: add main route stops to y domain no matter what
	// TODO: add to common stops if theres a transfer to the route
	const data = $derived.by(() => {
		const route_trips = [];
		// Track stops by route ID
		const stopsByRoute = new Map<string, Set<{ id: number; name: string; sequence: number }>>();

		// Initialize sets for each selected route
		for (const route of routes) {
			stopsByRoute.set(route.id, new Set());
		}

		for (const trip of trips.trips.values()) {
			if (routes.every((r) => r.id !== trip.route_id) || trip.direction !== direction) continue;
			const trip_st = stop_times.by_trip_id[trip.id];
			if (!trip_st) continue;

			// Skip trips that aren't fully complete (any departure time > current_time)
			// if (trip_st.some((st) => st.departure.getTime() <= current_time.ms)) continue;

			const trip_points = [];
			for (const st of trip_st) {
				if (st.arrival.getTime() < current_time.ms) continue;
				const stop = page.data.stops[st.stop_id];

				const stop_sequence = stop.routes.find((r) => r.id === trip.route_id)?.stop_sequence;
				if (!stop_sequence) {
					console.log(
						'stop_sequence not found, skipping',
						stop.name,
						trip.route_id,
						stop.routes.map((r) => r.id)
					);
					continue;
				}

				// Add the stop to its route's set
				const stopInfo = { id: stop.id, name: stop.name, sequence: stop_sequence };
				const routeStops = stopsByRoute.get(trip.route_id);
				if (routeStops) {
					routeStops.add(stopInfo);
				}

				trip_points.push({
					stop_id: stop.id,
					stop_name: stop.name,
					time: st.arrival
				});
			}

			route_trips.push({ trip, points: trip_points });
		}

		// Find stops that are common to all selected routes
		let commonStops: Array<{ id: number; name: string; sequence: number }> = [];

		if (routes.length > 0) {
			// Start with all stops from the first route
			const firstRouteStops = stopsByRoute.get(routes[0].id);
			if (firstRouteStops && firstRouteStops.size > 0) {
				commonStops = Array.from(firstRouteStops);

				// Filter to keep only stops that exist in all other routes
				for (let i = 1; i < routes.length; i++) {
					const routeStops = stopsByRoute.get(routes[i].id);
					if (routeStops) {
						commonStops = commonStops.filter((stop) =>
							Array.from(routeStops).some((rs) => rs.id === stop.id)
						);
					}
				}
			}

			// remove stops that are not in the main route from trip points
			route_trips.forEach((trip) => {
				trip.points = trip.points.filter((point) =>
					commonStops.some((stop) => stop.id === point.stop_id)
				);
			});
		}

		// Sort stops by sequence number
		commonStops.sort((a, b) => a.sequence - b.sequence);

		// Create y domain from the sorted common stops
		const yDomain = commonStops.map((stop) => stop.name);

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
		const filename = `${routes.map((r) => r.short_name).join('_')}_${direction === TripDirection.North ? 'northbound' : 'southbound'}_chart.svg`;

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

	// Sorted routes for the combobox
	const sortedRoutes = $derived(
		Object.values(page.data.routes).sort((a, b) => {
			// Train routes come first
			if (a.route_type === 'train' && b.route_type !== 'train') return -1;
			if (a.route_type !== 'train' && b.route_type === 'train') return 1;
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
		// Check if the route is already selected
		if (!routes.some((r) => r.id === selectedRoute.id)) {
			routes.push(selectedRoute);
		}
		isComboboxOpen = false;
		searchQuery = ''; // Clear search when selection is made
	}

	// Remove a route from selection
	function removeRoute(routeToRemove: Route) {
		// Don't remove if it's the last route
		if (routes.length <= 1) return;
		routes = routes.filter((r) => r.id !== routeToRemove.id);
	}

	// Check if route is already selected
	function isRouteSelected(routeId: string) {
		return routes.some((r) => r.id === routeId);
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
	$inspect(xDomain);
</script>

<svelte:head>
	<title>Charts | Train Status</title>
</svelte:head>

<div class="flex h-[calc(100dvh-8rem)] min-h-[300px] flex-col">
	<div class="pl-3 text-xl font-bold">Charts</div>
	<div
		class="mx-auto flex w-fit flex-wrap gap-6 rounded-md border border-neutral-700/50 bg-neutral-800/70 p-4"
	>
		<!-- Routes Option -->
		<div class="flex min-w-[240px] flex-col gap-2">
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
						{#if routes.length === 0}
							<span>Select routes</span>
						{:else}
							{#each routes as selectedRoute}
								<div
									class="flex items-center rounded-md border border-neutral-700 bg-neutral-800 px-2 py-1"
								>
									<Icon height={20} width={20} express={false} route={selectedRoute} link={false} />
									{#if routes.length > 1}
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
					<ChevronDown class="h-4 w-4 flex-shrink-0" />
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
										aria-selected={isRouteSelected(routeOption.id)}
										class="flex cursor-pointer items-center gap-2 px-4 py-2 hover:bg-neutral-800 focus:bg-neutral-800 focus:outline-none {isRouteSelected(
											routeOption.id
										)
											? 'bg-neutral-700'
											: ''}"
										onclick={() => selectRoute(routeOption)}
										onkeydown={(e) => handleOptionKeydown(e, routeOption)}
									>
										<Icon
											height={32}
											width={32}
											express={false}
											route={routeOption}
											link={false}
											class="mr-3 flex-shrink-0"
										/>
										<span class="flex-grow">{routeOption.long_name}</span>
										<!-- <div class="flex flex-col">
											<span class="text-sm">{getRouteDisplayName(routeOption)}</span>
											<span class="text-neutral-400 text-sm">{routeOption.long_name}</span>
										</div> -->
										{#if isRouteSelected(routeOption.id)}
											<Check class="mt-1 size-4 flex-shrink-0 text-green-500" />
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
						bind:group={direction}
						type="radio"
						id="northbound"
						name="direction"
						value={TripDirection.North}
						class="size-5 cursor-pointer transition-transform hover:scale-110"
					/>
				</div>
				<div class="flex items-center justify-between gap-2">
					<label for="southbound" class="cursor-pointer">Southbound</label>
					<input
						bind:group={direction}
						type="radio"
						id="southbound"
						name="direction"
						value={TripDirection.South}
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
		<div class="flex min-w-[120px] flex-col justify-center">
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
				<div bind:this={svgContainer} class="h-full min-h-[500px] w-full min-w-[1300px]">
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
							<Lines {routes} bind:stop_points />
						</Svg>
					</LayerCake>
				</div>
			</div>
		{:else}
			<div class="flex h-full items-center justify-center text-neutral-400">No data available</div>
		{/if}
	</div>
</div>
