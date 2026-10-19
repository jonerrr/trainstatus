<script lang="ts">
	import { Accessibility } from '@lucide/svelte';

	import type { ArrivalExits } from './platform-exits.js';

	let { guidance }: { guidance: ArrivalExits } = $props();
	let selected = $state<string | null>(null);
	let width = $state(360);
	let label_heights = $state<Record<string, number>>({});
	const selected_exit = $derived(guidance.exits.find((exit) => exit.id === selected));
	const train_width = $derived(Math.max(0, Math.min(width - 16, guidance.car_count * 36)));
	const train_left = $derived((width - train_width) / 2);
	const car_width = $derived(train_width / guidance.car_count - 2);
	const label_width = $derived(Math.min(112, width));
	const labels = $derived.by(() => {
		const lanes: number[] = [];
		const positions = guidance.exits.map((exit) => {
			const x = train_left + exit.position * train_width;
			const left = Math.max(0, Math.min(width - label_width, x - label_width / 2));
			let lane = lanes.findIndex((right) => right + 8 <= left);
			if (lane < 0) lane = lanes.length;
			lanes[lane] = left + label_width;
			return { ...exit, x, left, lane };
		});
		const heights = lanes.map(() => 44);
		for (const exit of positions)
			heights[exit.lane] = Math.max(heights[exit.lane], label_heights[exit.id] ?? 44);
		const tops: number[] = [];
		let top = 58;
		for (const height of heights) {
			tops.push(top);
			top += height + 8;
		}
		return positions.map((exit) => ({ ...exit, y: tops[exit.lane] }));
	});
	const height = $derived(
		Math.max(...labels.map((label) => label.y + (label_heights[label.id] ?? 44))) + 8
	);
</script>

<section
	aria-label="Platform exits at arrival"
	class="mx-auto mb-3 w-[calc(100%-2rem)] max-w-md rounded-lg bg-sky-950/30 px-3 pt-2 pb-3"
>
	<div bind:clientWidth={width}>
		<div
			class="mb-1 flex justify-between text-[0.65rem] text-neutral-400"
			style:margin-left={`${train_left}px`}
			style:width={`${train_width}px`}
			aria-hidden="true"
		>
			<span>Rear</span><span>Front →</span>
		</div>
		<div class="relative" style:height={`${height}px`}>
			<svg
				viewBox={`0 0 ${width} ${height}`}
				class="absolute inset-0 h-full w-full overflow-visible text-sky-300"
				aria-hidden="true"
			>
				{#each { length: guidance.car_count }, i (i)}
					{const car = $derived(guidance.car_count - i)}
					{const x = $derived(train_left + (i * train_width) / guidance.car_count + 1)}
					{#if i === guidance.car_count - 1}
						{const radius = $derived(Math.min(12, car_width / 2))}
						<path
							data-train-car={car}
							d={`M${x} 4 H${x + car_width - radius} Q${x + car_width} 4 ${x + car_width} 16 Q${x + car_width} 28 ${x + car_width - radius} 28 H${x} Z`}
							fill="currentColor"
							opacity={selected_exit?.car === car ? 1 : 0.65}
						/>
					{:else}
						<rect
							data-train-car={car}
							{x}
							y="4"
							width={car_width}
							height="24"
							rx="2"
							fill="currentColor"
							opacity={selected_exit?.car === car ? 1 : 0.65}
						/>
					{/if}
				{/each}
				{#each labels as exit (exit.id)}
					<line
						data-exit-shaft
						x1={exit.x}
						x2={exit.x}
						y1="34"
						y2="49"
						stroke="currentColor"
						stroke-width="2"
					/>
					<path
						d={`M${exit.x - 4} 44 L${exit.x} 49 L${exit.x + 4} 44`}
						fill="none"
						stroke="currentColor"
						stroke-width="2"
					/>
				{/each}
			</svg>
			{#each labels as exit (exit.id)}
				<button
					bind:clientHeight={label_heights[exit.id]}
					type="button"
					aria-label={`${exit.name}${exit.accessible ? ', accessible exit' : ''}, around car ${exit.car} from the front`}
					aria-pressed={selected === exit.id}
					onclick={() => {
						selected = exit.id;
					}}
					class={[
						'absolute flex min-h-11 flex-col items-center justify-start rounded px-1 py-1 text-center text-xs leading-snug text-sky-100 hover:bg-sky-900/40 focus-visible:outline-2 focus-visible:outline-sky-300',
						selected === exit.id && 'bg-sky-900/40'
					]}
					style:left={`${exit.left}px`}
					style:width={`${label_width}px`}
					style:top={`${exit.y}px`}
				>
					{#if exit.accessible}
						<span aria-hidden="true" class="mb-0.5 rounded bg-sky-600 p-0.5 text-white">
							<Accessibility size={16} />
						</span>
					{/if}
					<span class="wrap-anywhere">{exit.name}</span>
				</button>
			{/each}
		</div>
	</div>
	{#if selected_exit}
		<p class="mt-1 text-center text-xs text-sky-200" role="status">
			{selected_exit.name} · car {selected_exit.car} from the front
		</p>
	{/if}
</section>
