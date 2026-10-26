<script lang="ts">
	import type { ServiceTrip } from '#lib/client/index.js';
	import Icon from '#lib/Route/Icon.svelte';
	import type { DepartureOption } from '#lib/transit/nearby.js';

	import { Accessibility } from '@lucide/svelte';

	import type { ArrivalExits } from './platform-exits.js';

	let {
		guidance,
		trip,
		transfer
	}: {
		guidance: ArrivalExits | null;
		trip: ServiceTrip;
		transfer?: DepartureOption;
	} = $props();

	// TODO: we are going to need a better way to separate UI between different sources
	let label_heights = $state<Record<string, number>>({});
	const cars = $derived(trip.data.source === 'mta_subway' ? trip.data.consist_cars : []);
	const car_count = $derived(
		trip.data.source === 'mta_subway' ? (trip.data.consist?.car_count ?? cars.length) : 0
	);
	const width = $derived(car_count * 54 + 12);
	const train_width = $derived(car_count * 54 - 2);
	const label_width = 112;
	const labels = $derived.by(() => {
		const lanes: number[] = [];
		const positions = (guidance?.exits ?? []).map((exit) => {
			const x = 6 + exit.position * train_width;
			const left = Math.max(0, Math.min(width - label_width, x - label_width / 2));
			let lane = lanes.findIndex((right) => right + 8 <= left);
			if (lane < 0) lane = lanes.length;
			lanes[lane] = left + label_width;
			return { ...exit, x, left, lane };
		});
		const heights = lanes.map(() => 30);
		for (const exit of positions)
			heights[exit.lane] = Math.max(heights[exit.lane], label_heights[exit.id] ?? 30);
		const tops: number[] = [];
		let top = 38;
		for (const height of heights) {
			tops.push(top);
			top += height + 4;
		}
		return positions.map((exit) => ({ ...exit, y: tops[exit.lane] }));
	});
	const height = $derived(
		labels.length
			? Math.max(...labels.map((label) => label.y + (label_heights[label.id] ?? 30))) + 4
			: 30
	);

	function center_transfer(viewport: HTMLDivElement) {
		if (!transfer || !guidance?.exits.length) return;
		const positions = guidance.exits.map((exit) => 6 + exit.position * train_width);
		const center = (Math.min(...positions) + Math.max(...positions)) / 2;
		viewport.scrollTo({
			left: Math.max(0, center - viewport.clientWidth / 2),
			behavior: 'instant'
		});
	}
</script>

{#if car_count > 0}
	<section class="platform" aria-label="Platform exits at arrival">
		<!-- Keyboard focus enables native horizontal scrolling. -->
		<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
		<div
			class="viewport"
			{@attach center_transfer}
			tabindex="0"
			role="region"
			aria-label="Train and platform exits; scroll for more"
		>
			<div class="diagram" style:width={`${width}px`} style:height={`${height}px`}>
				<svg
					viewBox={`0 0 ${width} ${height}`}
					aria-label="Train cars; front cab at the right"
					role="img"
				>
					{#each Array.from({ length: car_count }, (_, i) => car_count - i) as number (number)}
						{const i = car_count - number}
						{const car = cars[number - 1]}
						<g
							transform={`translate(${6 + i * 54}, 0)`}
							data-train-car={number}
							data-highlighted={!!guidance?.exits.some((exit) => exit.car === number)}
						>
							<title
								>{car
									? `Car ${number} · #${car.number}${car.type ? ` · ${car.type}` : ''}`
									: `Car ${number} from the front`}</title
							>
							<path
								class="body"
								d={number === 1
									? 'M3 2H42L51 9V23Q51 25 49 25H3Q1 25 1 23V4Q1 2 3 2Z'
									: 'M3 2H49Q51 2 51 4V23Q51 25 49 25H3Q1 25 1 23V4Q1 2 3 2Z'}
							/>
							{#if car?.number && car.number !== String(number)}
								<text class="car-pos" x="25" y="12" text-anchor="middle">{number}</text>
								<text class="car-id" x="25" y="22.5" text-anchor="middle">{car.number}</text>
							{:else}
								<text class="car-pos" x="25" y="16" text-anchor="middle">{number}</text>
							{/if}
						</g>
					{/each}
					{#each labels as exit (exit.id)}
						<path
							data-exit-shaft
							d={`M${exit.x} 25V33H${exit.left + label_width / 2}V${exit.y - 2}`}
							fill="none"
							stroke={transfer ? 'var(--accent)' : 'currentColor'}
						/>
					{/each}
				</svg>
				<ul>
					{#each labels as exit (exit.id)}
						<li
							bind:clientHeight={label_heights[exit.id]}
							aria-label={`${exit.name}${transfer ? ` to ${transfer.route.short_name}` : ''}${exit.accessible ? ', accessible path' : ''}, around car ${exit.car} from the front`}
							style:left={`${exit.left}px`}
							style:top={`${exit.y}px`}
							style:width={`${label_width}px`}
						>
							{#if exit.accessible}<Accessibility
									size={13}
									aria-label="Provider marks this path accessible"
								/>{/if}
							<span>{exit.name}</span>{#if transfer}<Icon
									route={transfer.route}
									width={16}
									height={16}
									link={false}
								/>{/if}
						</li>
					{/each}
				</ul>
			</div>
		</div>
	</section>
{/if}

<style>
	.platform {
		display: flex;
		flex-direction: column;
		flex: 0 1 auto;
		min-height: 30px;
	}
	.viewport {
		flex: 0 1 auto;
		min-height: 0;
		max-height: min(140px, 20dvh);
		overflow: auto;
		overscroll-behavior: contain;
		scrollbar-width: thin;
		scrollbar-color: var(--border-strong) transparent;
	}
	.viewport:focus-visible {
		outline: 2px solid var(--accent);
		outline-offset: -2px;
	}
	.diagram {
		position: relative;
		margin-inline: auto;
		color: var(--text-disabled);
	}
	svg {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
	}
	.body {
		fill: var(--bg-surface-2);
		stroke: var(--text-disabled);
	}
	.car-pos {
		fill: var(--text-secondary);
		font-size: 9.5px;
		font-weight: 700;
		font-family: inherit;
		font-variant-numeric: tabular-nums;
	}
	.car-id {
		fill: #8a8a8a;
		font-size: 7.5px;
		font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
		font-variant-numeric: tabular-nums;
		letter-spacing: -0.02em;
	}
	[data-highlighted='true'] .body {
		fill: var(--accent-car-fill);
		stroke: var(--accent);
		stroke-width: 1.5px;
	}
	[data-highlighted='true'] .car-pos {
		fill: #ffffff;
	}
	[data-highlighted='true'] .car-id {
		fill: var(--accent-hover);
	}
	ul {
		list-style: none;
	}
	li {
		position: absolute;
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		align-items: center;
		gap: 3px;
		padding: 0 2px 2px;
		color: var(--text-secondary);
		text-align: center;
		font-size: 11px;
		line-height: 1.3;
		overflow-wrap: anywhere;
	}
</style>
