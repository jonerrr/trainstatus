<script lang="ts">
	import { untrack } from 'svelte';

	import { SvelteMap } from 'svelte/reactivity';

	import { page } from '$app/state';

	import type { Source } from '#lib/client/index.js';

	import DeckOverlay from './DeckOverlay.svelte';
	import type { VehiclePicker } from './interactions';
	import { watchTrajectories, type TrajectorySnapshot } from './trajectories';
	import {
		buildActiveVehiclesAtTime,
		type ActiveVehicle,
		type RenderUnitTable
	} from './trajectoryArrow';
	import { isRailVehicle, vehicleLayers } from './vehicleLayers';

	let {
		sources,
		railDetail = false,
		busDetail = false,
		selectedTrip = null,
		onPickerReady
	}: {
		sources: Source[];
		railDetail?: boolean;
		busDetail?: boolean;
		selectedTrip?: string | null;
		onPickerReady: (picker: VehiclePicker | null) => void;
	} = $props();
	const fixedAt = $derived.by(() => {
		const value = page.url.searchParams.get('at');
		return value !== null && Number.isFinite(Number(value)) ? Number(value) : null;
	});
	let snapshot = $state.raw<TrajectorySnapshot>({ tables: new Map(), errors: new Map() });
	const invalidTables = new WeakSet<RenderUnitTable>();
	const identities = new WeakMap<ActiveVehicle[], string[]>();
	const pools = new SvelteMap<Source, ActiveVehicle[]>();
	let frame = $state(0);
	let membership = $state(0);
	let decodeError = $state(false);

	$effect(() =>
		watchTrajectories({ sources, at: fixedAt, refreshInterval: 30_000 }, (next) => {
			snapshot = next;
		})
	);

	function update(time: number) {
		let changed = false;
		for (const source of pools.keys()) {
			if (!snapshot.tables.has(source)) {
				pools.delete(source);
				changed = true;
			}
		}
		for (const [source, table] of snapshot.tables) {
			const pool = pools.get(source) ?? [];
			const previousIds = identities.get(pool) ?? [];
			identities.set(pool, previousIds);
			try {
				if (invalidTables.has(table)) {
					pool.length = 0;
					decodeError = true;
				} else buildActiveVehiclesAtTime(table, time, pool);
			} catch {
				invalidTables.add(table);
				pool.length = 0;
				decodeError = true;
			}
			// Reuse the identity buffer rather than allocating arrays/strings every frame.
			if (previousIds.length !== pool.length) changed = true;
			for (let i = 0; i < pool.length; i++) {
				if (previousIds[i] !== pool[i].renderUnitId) changed = true;
				previousIds[i] = pool[i].renderUnitId;
			}
			previousIds.length = pool.length;
			pools.set(source, pool);
		}
		if (changed) membership++;
		frame++;
	}

	$effect(() => {
		void snapshot;
		const at = fixedAt;
		untrack(() => {
			decodeError = false;
			update(at ?? Date.now() / 1000);
			membership++;
		});
		if (at !== null) return;
		let previous = 0;
		let id: number;
		function animate(now: number) {
			if (now - previous >= 1000 / 30) {
				update(Date.now() / 1000);
				previous = now;
			}
			id = requestAnimationFrame(animate);
		}
		id = requestAnimationFrame(animate);
		return () => cancelAnimationFrame(id);
	});
	const partitions = $derived.by(() => {
		void membership;
		const detail: ActiveVehicle[] = [];
		const puck: ActiveVehicle[] = [];
		for (const source of sources)
			for (const vehicle of pools.get(source) ?? []) {
				if (isRailVehicle(vehicle) ? railDetail : busDetail) detail.push(vehicle);
				else if (!isRailVehicle(vehicle) || vehicle.isHead) puck.push(vehicle);
			}
		return { detail, puck };
	});
	const layers = $derived(vehicleLayers(partitions, selectedTrip, frame, membership));
</script>

<DeckOverlay {layers} onpicker={onPickerReady} />
{#if snapshot.errors.size || decodeError}
	<div
		class="pointer-events-none absolute right-3 bottom-14 z-20 rounded-lg bg-neutral-950/90 px-3 py-2 text-xs text-amber-200"
		role="status"
	>
		Some vehicle data is unavailable. {fixedAt === null
			? 'Retrying automatically.'
			: 'Try another time or reload.'}
	</div>
{/if}
