<script lang="ts">
	import { onDestroy, onMount, untrack } from 'svelte';

	import { page } from '$app/state';

	import type { Source } from '$lib/client';
	import { trip_context } from '$lib/resources/trips.svelte';
	import { open_modal } from '$lib/url_params.svelte';

	import type { PickingInfo } from '@deck.gl/core';
	import { IconLayer } from '@deck.gl/layers';
	import { DeckGLOverlay } from '@svelte-maplibre-gl/deckgl';

	import {
		buildActiveVehiclesAtTime,
		normalizeBearingForIcon,
		renderUnitTableFromIPC,
		type ActiveVehicle,
		type RenderUnitTable
	} from '$lib/map/trajectoryArrow';
	import { VEHICLE_ICON_ATLAS, VEHICLE_ICON_MAPPING } from '$lib/map/vehicleIcons';

	let {
		source = 'mta_subway',
		refreshInterval = 30_000,
		enabled = true
	}: {
		source?: Source;
		routeIds?: string[];
		refreshInterval?: number;
		enabled?: boolean;
	} = $props();

	const tripResource = $derived(trip_context.getSource(source));

	const fixedAt = $derived.by(() => {
		const atParam = page.url.searchParams.get('at');
		if (!atParam) return null;
		const parsed = Number(atParam);
		return Number.isFinite(parsed) ? parsed : null;
	});

	let renderUnits = $state<RenderUnitTable | null>(null);
	let activeVehicles: ActiveVehicle[] = [];
	let activeVehicleCount = $state(0);
	let frameVersion = $state(0);
	let animationId: number | undefined;
	let fetchTimer: ReturnType<typeof setInterval> | undefined;

	const deckLayers = $derived.by(() => {
		if (!enabled || activeVehicleCount === 0) return [];

		const railVehicles = activeVehicles.filter((vehicle) => isRailVehicle(vehicle));
		const busVehicles = activeVehicles.filter((vehicle) => !isRailVehicle(vehicle));

		return [
			new IconLayer<ActiveVehicle>({
				id: `trip-markers-rail-outline-${source}`,
				data: railVehicles,
				iconAtlas: VEHICLE_ICON_ATLAS,
				iconMapping: VEHICLE_ICON_MAPPING,
				getIcon: (vehicle) => vehicle.iconKey,
				getPosition: (vehicle) => vehicle.position,
				getColor: (vehicle) => getVehicleOutlineColor(vehicle),
				getAngle: (vehicle) => normalizeBearingForIcon(vehicle.bearing),
				getSize: (vehicle) => vehicle.lengthM * 1.18,
				sizeUnits: 'meters',
				billboard: false,
				alphaCutoff: 0,
				sizeMinPixels: 9,
				pickable: false,
				updateTriggers: {
					getPosition: frameVersion,
					getAngle: frameVersion,
					getColor: frameVersion,
					getSize: frameVersion,
					getIcon: frameVersion
				}
			}),
			new IconLayer<ActiveVehicle>({
				id: `trip-markers-rail-fill-${source}`,
				data: railVehicles,
				iconAtlas: VEHICLE_ICON_ATLAS,
				iconMapping: VEHICLE_ICON_MAPPING,
				getIcon: (vehicle) => vehicle.iconKey,
				getPosition: (vehicle) => vehicle.position,
				getColor: (vehicle) => getVehicleFillColor(vehicle),
				getAngle: (vehicle) => normalizeBearingForIcon(vehicle.bearing),
				getSize: (vehicle) => vehicle.lengthM * 0.94,
				sizeUnits: 'meters',
				billboard: false,
				alphaCutoff: 0,
				sizeMinPixels: 7,
				pickable: true,
				updateTriggers: {
					getPosition: frameVersion,
					getAngle: frameVersion,
					getColor: frameVersion,
					getSize: frameVersion,
					getIcon: frameVersion
				}
			}),
			new IconLayer<ActiveVehicle>({
				id: `trip-markers-bus-${source}`,
				data: busVehicles,
				iconAtlas: VEHICLE_ICON_ATLAS,
				iconMapping: VEHICLE_ICON_MAPPING,
				getIcon: (vehicle) => vehicle.iconKey,
				getPosition: (vehicle) => vehicle.position,
				getColor: (vehicle) => vehicle.color,
				getAngle: (vehicle) => normalizeBearingForIcon(vehicle.bearing),
				getSize: (vehicle) => vehicle.lengthM,
				sizeUnits: 'meters',
				billboard: false,
				alphaCutoff: 0,
				sizeMinPixels: 8,
				pickable: true,
				updateTriggers: {
					getPosition: frameVersion,
					getAngle: frameVersion,
					getColor: frameVersion,
					getSize: frameVersion,
					getIcon: frameVersion
				}
			})
		];
	});

	function isRailVehicle(vehicle: ActiveVehicle) {
		return vehicle.iconKey === 'rail_head' || vehicle.iconKey === 'rail_car';
	}

	function getVehicleFillColor(vehicle: ActiveVehicle): [number, number, number, number] {
		if (isRailVehicle(vehicle)) {
			return vehicle.isHead ? [248, 248, 244, 245] : [228, 230, 232, 240];
		}

		return vehicle.color;
	}

	function getVehicleOutlineColor(vehicle: ActiveVehicle): [number, number, number, number] {
		return [vehicle.color[0], vehicle.color[1], vehicle.color[2], 255];
	}

	function clearActiveVehicles() {
		activeVehicles.length = 0;
		activeVehicleCount = 0;
		frameVersion += 1;
	}

	function updateVehiclesAtTime(table: RenderUnitTable | null, t: number) {
		if (!table) {
			clearActiveVehicles();
			return;
		}

		activeVehicleCount = buildActiveVehiclesAtTime(table, t, activeVehicles).length;
		frameVersion += 1;
	}

	async function openTripModal(tripId: string) {
		const trip = tripResource?.current?.get(tripId);
		if (trip) {
			open_modal({ type: 'trip', ...trip });
			return;
		}

		if (!tripResource) return;

		try {
			const trips = await tripResource.whenReady();
			const readyTrip = trips.get(tripId);
			if (readyTrip) {
				open_modal({ type: 'trip', ...readyTrip });
			}
		} catch (err) {
			console.error('Unable to resolve trip for marker click:', err);
		}
	}

	function handleDeckClick(info: PickingInfo<ActiveVehicle>) {
		const tripId = info.object?.tripId;
		if (tripId) {
			void openTripModal(tripId);
		}
	}

	async function fetchTrajectories(
		currentSource: Source,
		currentFixedAt: number | null,
		currentEnabled: boolean
	) {
		if (!currentEnabled) {
			renderUnits = null;
			clearActiveVehicles();
			return;
		}

		try {
			const params = new URLSearchParams();
			if (currentFixedAt !== null) params.set('at', String(currentFixedAt));

			const queryString = params.toString();
			const url = `/api/v1/trajectories/${currentSource}${queryString ? `?${queryString}` : ''}`;

			const res = await fetch(url);
			if (!res.ok) {
				console.error(`Failed to fetch trajectories: ${res.status}`);
				return;
			}

			const nextRenderUnits = renderUnitTableFromIPC(await res.arrayBuffer());
			renderUnits = nextRenderUnits;
			updateVehiclesAtTime(nextRenderUnits, currentFixedAt ?? Date.now() / 1000);

			if (nextRenderUnits.table.numRows > 0 && activeVehicleCount === 0) {
				console.warn(
					`[TripMarkers] ${nextRenderUnits.table.numRows} render units loaded but none were active at t=${currentFixedAt ?? Date.now() / 1000}`
				);
			}
		} catch (err) {
			console.error('Error fetching trajectories:', err);
		}
	}

	let isRunning = false;

	function animate() {
		const currentRenderUnits = renderUnits;
		if (!enabled || !currentRenderUnits || fixedAt !== null) {
			isRunning = false;
			animationId = undefined;
			return;
		}
		updateVehiclesAtTime(currentRenderUnits, Date.now() / 1000);
		animationId = requestAnimationFrame(animate);
	}

	function startAnimation() {
		if (!isRunning && enabled && renderUnits && fixedAt === null) {
			isRunning = true;
			animationId = requestAnimationFrame(animate);
		}
	}

	function stopAnimation() {
		if (isRunning) {
			isRunning = false;
			if (animationId !== undefined) {
				cancelAnimationFrame(animationId);
				animationId = undefined;
			}
		}
	}

	$effect(() => {
		const currentEnabled = enabled;
		const currentRenderUnits = renderUnits;
		const currentFixedAt = fixedAt;

		if (currentEnabled && currentRenderUnits && currentFixedAt === null) {
			startAnimation();
		} else {
			stopAnimation();
		}
	});

	$effect(() => {
		const currentFixedAt = fixedAt;
		const currentRenderUnits = renderUnits;
		if (currentRenderUnits) {
			untrack(() => {
				updateVehiclesAtTime(currentRenderUnits, currentFixedAt ?? Date.now() / 1000);
			});
		}
	});

	$effect(() => {
		const currentEnabled = enabled;
		const currentSource = source;
		const currentFixedAt = fixedAt;
		untrack(() => {
			if (!currentEnabled) {
				renderUnits = null;
				clearActiveVehicles();
				return;
			}
			void fetchTrajectories(currentSource, currentFixedAt, currentEnabled);
		});
	});

	$effect(() => {
		const currentEnabled = enabled;
		const currentSource = source;
		const currentFixedAt = fixedAt;
		const currentRefreshInterval = refreshInterval;

		if (fetchTimer !== undefined) {
			clearInterval(fetchTimer);
			fetchTimer = undefined;
		}

		if (!currentEnabled || currentFixedAt !== null) return;

		fetchTimer = setInterval(() => {
			void fetchTrajectories(currentSource, null, currentEnabled);
		}, currentRefreshInterval);

		return () => {
			if (fetchTimer !== undefined) {
				clearInterval(fetchTimer);
				fetchTimer = undefined;
			}
		};
	});

	onDestroy(() => {
		stopAnimation();
		if (fetchTimer !== undefined) clearInterval(fetchTimer);
	});
</script>

{#if enabled}
	<DeckGLOverlay interleaved layers={deckLayers} onClick={handleDeckClick} />
{/if}
