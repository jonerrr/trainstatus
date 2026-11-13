<script lang="ts">
	import { onMount } from 'svelte';

	import { page } from '$app/state';

	import AxisX from '#lib/charts/AxisX.svelte';
	import AxisY from '#lib/charts/AxisY.svelte';
	import { buildChartData, chartXDomain, type ChartInput } from '#lib/charts/data.js';
	import Lines from '#lib/charts/Lines.svelte';
	import type { CatalogRoute, Source } from '#lib/client/index.js';
	import { getTransitData } from '#lib/data/store.svelte.js';
	import Icon from '#lib/route/Icon.svelte';
	import Skeleton from '#lib/Skeleton.svelte';
	import { getCurrentTime } from '#lib/url-params.svelte.js';

	import { Check, ChevronDown, Download, Search, X } from '@lucide/svelte';
	import { scalePoint, scaleTime } from 'd3-scale';
	import { flatten, LayerCake, Svg } from 'layercake';

	const currentTime = getCurrentTime();
	const transit = getTransitData();

	// TODO: maybe somehow include the linecharts in the stop/trip/route modals

	const catalogResources = transit.catalogs(() => page.data.selectedSources);
	const catalogs = $derived(catalogResources.current.filter((r) => r.state === 'ready'));

	type RouteKey = { source: Source; id: string };
	let selected = $state<RouteKey[]>(
		page.data.selectedSources.includes('mta_subway') ? [{ source: 'mta_subway', id: '4' }] : []
	);

	// 0 = first direction (subway northbound / bus dir 0)
	// 1 = second direction (subway southbound / bus dir 1)
	let directionIndex = $state(0);

	// Per-source direction values: subway uses 1 (north) / 3 (south), bus uses 0 / 1
	const directionOf = (source: Source) =>
		source === 'mta_subway' ? (directionIndex === 0 ? 1 : 3) : directionIndex;

	// Selected routes present in their source catalogs (for Lines, export filename, etc.)
	const selectedRoutesFlat = $derived(
		selected.flatMap(({ source, id }) => {
			const route = catalogs.find((c) => c.source === source)?.value?.routesById.get(id);
			return route ? [route] : [];
		})
	);

	const routeResources = transit.routes(() =>
		selected.map((key) => ({
			source: key.source,
			routeId: key.id,
			at: currentTime.value
		}))
	);
	const reads = $derived(routeResources.current);
	const tripsLoading = $derived(
		catalogResources.current.some((c) => c.state === 'loading') ||
			reads.some((r) => r.state === 'loading')
	);

	let stopPoints = $state<boolean>(false);
	let currentTimeLine = $state<boolean>(true);
	// TODO: fix x axis interval causing page to freeze
	let xAxisInterval = $state<number>(15);
	let displayHours = $state<number>(3);

	const data = $derived.by(() => {
		const inputs: ChartInput[] = [];
		for (const read of reads) {
			if (read.state !== 'ready') continue;
			const { source, routeId: id, value: route } = read;
			const stops = catalogs.find((c) => c.source === source)?.value?.stopsById;
			if (!route || !stops) continue;
			for (const trip of route.trips.values()) {
				if (trip.direction !== directionOf(source)) continue;
				const calls = route.callsByTrip.get(trip.id);
				if (calls?.length) inputs.push({ source, routeId: id, trip, calls, stops });
			}
		}
		return buildChartData(inputs);
	});

	const visibleTrips = $derived(
		data.routeTrips.filter(
			({ points }) =>
				points[0].time.getTime() <= xDomain[1] &&
				points[points.length - 1].time.getTime() >= xDomain[0]
		)
	);

	const chartPadding = { top: 20, right: 10, left: 160, bottom: 44 };

	// Create a reference to the SVG element
	let svgContainer = $state<HTMLDivElement>();

	/** Download a standalone chart, expanding its layout bounds to contain rendered labels. */
	function exportAsSvg() {
		if (!svgContainer) return;

		// Find the SVG element inside the container
		const svgElement = svgContainer.querySelector('svg');
		if (!svgElement) return;

		// Clone the SVG to avoid modifying the original
		const svgClone = svgElement.cloneNode(true) as SVGElement;

		// Add required attributes for standalone SVG
		svgClone.setAttribute('xmlns', 'http://www.w3.org/2000/svg');
		svgClone.setAttribute('version', '1.1');

		// Preserve the layout bounds and expand them for labels outside the fixed margins.
		const { width, height } = svgElement.getBoundingClientRect();
		const { top, right, bottom, left } = chartPadding;
		const labels = svgElement.querySelector<SVGGElement>('.y-axis')!.getBBox();
		const minX = Math.min(-left, labels.x);
		const minY = Math.min(-top, labels.y);
		const totalWidth = Math.max(width + right, labels.x + labels.width) - minX;
		const totalHeight = Math.max(height + bottom, labels.y + labels.height) - minY;
		svgClone.setAttribute('viewBox', `${minX} ${minY} ${totalWidth} ${totalHeight}`);
		svgClone.setAttribute('width', String(totalWidth));
		svgClone.setAttribute('height', String(totalHeight));
		svgClone.removeAttribute('style');

		const background = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
		background.setAttribute('x', String(minX));
		background.setAttribute('y', String(minY));
		background.setAttribute('width', String(totalWidth));
		background.setAttribute('height', String(totalHeight));
		background.setAttribute('fill', '#171717');

		// Insert background as first child so it's behind everything else
		svgClone.insertBefore(background, svgClone.firstChild);

		// Add watermark
		const watermark = document.createElementNS('http://www.w3.org/2000/svg', 'text');
		watermark.textContent = 'TrainStat.us';
		watermark.setAttribute('x', String(minX + 6));
		watermark.setAttribute('y', String(minY + 14));
		watermark.setAttribute('font-size', '16');
		watermark.setAttribute('font-weight', '600');
		watermark.setAttribute('fill', '#e5e5e5');

		// Add watermark to the SVG
		svgClone.appendChild(watermark);

		// Generate a clean SVG string
		const svgData = new XMLSerializer().serializeToString(svgClone);

		// Create a blob with the SVG data
		const blob = new Blob([svgData], { type: 'image/svg+xml' });

		// Create a download URL
		const url = URL.createObjectURL(blob);

		// Create a filename with all route short names
		const filename = `${selectedRoutesFlat.map((r) => r.short_name).join('_')}_${directionIndex === 0 ? 'northbound' : 'southbound'}_chart.svg`;

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
		catalogs
			.flatMap((c) => c.value.routes)
			.sort((a, b) => {
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
	function getRouteDisplayName(route: CatalogRoute) {
		return route.short_name === 'S' ? route.id : route.short_name;
	}

	// Close combobox when clicking outside
	function handleClickOutside(event: MouseEvent) {
		if (comboboxRef && !comboboxRef.contains(event.target as Node)) {
			isComboboxOpen = false;
		}
	}

	// Handle route selection
	function selectRoute(selectedRoute: CatalogRoute) {
		if (!isRouteSelected(selectedRoute)) {
			selected = [...selected, { source: selectedRoute.data.source, id: selectedRoute.id }];
		}
		isComboboxOpen = false;
		searchQuery = ''; // Clear search when selection is made
	}

	// Remove a route from selection
	function removeRoute(routeToRemove: CatalogRoute) {
		// Don't remove if it's the last route
		if (selected.length <= 1) return;
		selected = selected.filter(
			(key) => key.source !== routeToRemove.data.source || key.id !== routeToRemove.id
		);
	}

	// Check if route is already selected
	function isRouteSelected(route: CatalogRoute) {
		return selected.some((key) => key.source === route.data.source && key.id === route.id);
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
	function handleOptionKeydown(event: KeyboardEvent, routeOption: CatalogRoute) {
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

	// let last_at = currentTime.value;
	// TODO: convert this to an attachment
	onMount(() => {
		document.addEventListener('click', handleClickOutside);
		// set current time to 2 hours ago
		// currentTime.value = Math.floor((currentTime.ms - 4 * 60 * 60 * 1000) / 1000);

		return () => {
			document.removeEventListener('click', handleClickOutside);
			// if (last_at) {
			// 	console.log('setting to old current time', last_at);
			// 	currentTime.value = last_at;
			// }
		};
	});

	const xDomain = $derived(
		chartXDomain(data.routeTrips, currentTime.ms, displayHours * 60 * 60 * 1000)
	);
	// const xRange = $derived(() => {
	// 	const width = svgContainer?.clientWidth || 0;
	// 	return [0, width];
	// });
	// $inspect(xDomain);
</script>

<!-- <svelte:head>
	<title>Charts | Train Status</title>
</svelte:head> -->

<div class="flex h-full min-h-75 flex-col py-4">
	<div
		class="mx-auto flex w-fit flex-wrap gap-6 rounded-none border border-border-default/50 bg-surface-2/70 p-4"
	>
		<!-- Routes Option -->
		<!-- TODO: improve ui for mobile -->
		<!-- TODO: add tooltips for options (use attachments) -->
		<div class="flex min-w-60 flex-col gap-2">
			<div class="font-semibold">Routes</div>
			<div class="relative w-full" bind:this={comboboxRef}>
				<button
					type="button"
					onclick={toggleCombobox}
					aria-haspopup="listbox"
					aria-expanded={isComboboxOpen}
					class="flex w-full cursor-pointer items-center justify-between gap-2 rounded-none border border-border-default bg-surface-1 px-3 py-2 text-base hover:bg-hover"
					aria-label="Add routes"
				>
					<div class="flex flex-1 flex-wrap items-center gap-2">
						{#if selectedRoutesFlat.length === 0}
							<span>Select routes</span>
						{:else}
							{#each selectedRoutesFlat as selectedRoute (`${selectedRoute.data.source}:${selectedRoute.id}`)}
								<div
									class="flex items-center rounded-none border border-border-default bg-surface-2 px-2 py-1"
								>
									<Icon height={20} width={20} route={selectedRoute} link={false} />
									{#if selectedRoutesFlat.length > 1}
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
											class="ml-1 cursor-pointer text-muted hover:text-white"
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
						class="absolute z-10 mt-1 w-64 max-w-64 overflow-hidden rounded-none border border-border-default bg-surface-1 shadow-lg"
					>
						<div class="border-b border-border-default p-2">
							<div class="relative">
								<div class="pointer-events-none absolute inset-y-0 left-0 flex items-center pl-3">
									<Search class="h-4 w-4 text-muted" />
								</div>
								<input
									type="text"
									bind:this={searchInputRef}
									bind:value={searchQuery}
									onkeydown={handleComboboxKeydown}
									class="block w-full rounded-none border border-border-default bg-surface-2 py-2 pr-3 pl-10 text-sm placeholder-muted"
									placeholder="Search routes..."
									autocomplete="off"
								/>
							</div>
						</div>

						<!-- Route options -->
						<div id="route-listbox" role="listbox" class="max-h-60 overflow-auto py-1">
							{#if filteredRoutes.length === 0}
								<div class="px-4 py-2 text-muted">No routes found</div>
							{:else}
								{#each filteredRoutes as routeOption (`${routeOption.data.source}:${routeOption.id}`)}
									<div
										role="option"
										tabindex="0"
										aria-selected={isRouteSelected(routeOption)}
										class="flex cursor-pointer items-center gap-2 px-4 py-2 hover:bg-hover focus:bg-hover focus:outline-none {isRouteSelected(
											routeOption
										)
											? 'bg-accent-bg text-accent'
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
											<span class="text-muted text-sm">{routeOption.long_name}</span>
										</div> -->
										{#if isRouteSelected(routeOption)}
											<Check class="mt-1 size-4 shrink-0 text-accent" />
											<!-- <span class="ml-auto text-accent">✓</span> -->
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
		<!-- TODO: change direction text depending on source -->
		<div class="flex min-w-24 flex-col gap-2">
			<div class="font-bold">Direction</div>
			<div class="flex flex-col gap-2 text-secondary">
				<div class="flex items-center justify-between gap-2">
					<label for="northbound" class="cursor-pointer">Northbound</label>
					<input
						bind:group={directionIndex}
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
						bind:group={directionIndex}
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
		<div class="flex min-w-24 flex-col gap-2">
			<div class="font-bold">Style</div>
			<div class="flex flex-col gap-2 text-secondary">
				<div class="flex w-full items-center justify-between gap-2">
					<label for="stop_points" class="cursor-pointer">Stop Points</label>
					<input
						id="stop_points"
						type="checkbox"
						name="stop_points"
						bind:checked={stopPoints}
						class="size-5 cursor-pointer transition-transform hover:scale-110"
					/>
				</div>
				<div class="flex w-full justify-between gap-2">
					<label for="time_line" class="cursor-pointer">Time Line</label>
					<input
						id="time_line"
						type="checkbox"
						name="time_line"
						bind:checked={currentTimeLine}
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
			<label for="display-hours" class="font-semibold">Hours Ahead</label>
			<div class="flex flex-col gap-1">
				<div class="flex items-center gap-2">
					<input
						id="display-hours"
						aria-describedby="chart-history-range"
						type="range"
						min="1"
						max="4"
						step="1"
						bind:value={displayHours}
						class="w-full cursor-pointer"
					/>
					<span class="w-8 text-right">{displayHours}h</span>
				</div>
				<p id="chart-history-range" class="max-w-48 text-xs text-muted">
					Also shows recorded history up to {displayHours}
					{displayHours === 1 ? 'hour' : 'hours'} back.
				</p>
			</div>
		</div>

		<!-- Export Button -->
		<div class="flex min-w-30 flex-col justify-center">
			<button
				onclick={exportAsSvg}
				class="flex h-fit items-center justify-center gap-2 rounded-none bg-surface-1 px-4 py-2 transition-colors hover:bg-hover"
				aria-label="Export chart as SVG"
			>
				<Download class="size-5" />
				Export SVG
			</button>
		</div>
	</div>

	<!-- Chart container with proper overflow handling -->
	<div class="relative flex-1 overflow-hidden">
		{#if visibleTrips.length && data.yDomain.length}
			<div class="absolute inset-0 overflow-auto">
				<div bind:this={svgContainer} class="h-full min-h-125 w-full min-w-325">
					<LayerCake
						debug={false}
						ssr
						padding={chartPadding}
						x="time"
						y="stopKey"
						{xDomain}
						yDomain={data.yDomain}
						yScale={scalePoint().padding(0)}
						xScale={scaleTime()}
						data={visibleTrips}
						flatData={flatten(visibleTrips, 'points')}
					>
						<Svg>
							<AxisX {currentTimeLine} interval={xAxisInterval} />
							<AxisY stopNames={data.stopNames} />
							<Lines routes={selectedRoutesFlat} bind:stopPoints />
						</Svg>
					</LayerCake>
				</div>
			</div>
		{:else if tripsLoading}
			<div class="flex h-full items-center justify-center">
				<Skeleton lines={8} class="w-3/4" />
			</div>
		{:else if !reads.some((r) => r.state === 'failed') && !catalogResources.current.some((r) => r.state === 'failed')}
			<div class="flex h-full items-center justify-center text-muted">No data available</div>
		{/if}
	</div>
</div>

<style>
	@reference "../../app.css";

	:global(.layercake-layout-svg) {
		@apply bg-canvas;
	}
</style>
