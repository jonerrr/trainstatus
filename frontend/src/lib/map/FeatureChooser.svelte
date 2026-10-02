<script lang="ts">
	import type { Attachment } from 'svelte/attachments';

	import MapBackdrop from './MapBackdrop.svelte';
	import { dismissOnEscape } from './dialog';
	import type { MapTarget, ScreenPoint } from './interactions';

	let {
		targets,
		point,
		viewportWidth,
		viewportHeight,
		onselect,
		ondismiss
	}: {
		targets: MapTarget[];
		point: ScreenPoint;
		viewportWidth: number;
		viewportHeight: number;
		onselect: (target: MapTarget) => void;
		ondismiss: () => void;
	} = $props();

	const left = $derived(Math.min(Math.max(8, point.x), Math.max(8, viewportWidth - 288)));
	const top = $derived(Math.min(Math.max(8, point.y), Math.max(8, viewportHeight - 260)));

	const focusFirstTarget: Attachment<HTMLDivElement> = (node) => {
		queueMicrotask(() => node.querySelector<HTMLButtonElement>('button')?.focus());
	};
</script>

<svelte:window
	onkeydown={(event) => {
		dismissOnEscape(event, ondismiss);
	}}
/>

<MapBackdrop class="chooser-backdrop" label="Dismiss feature chooser" {ondismiss} />

<div
	{@attach focusFirstTarget}
	class="chooser-panel"
	style:--chooser-left={`${left}px`}
	style:--chooser-top={`${top}px`}
	role="dialog"
	aria-modal="true"
	aria-label="Choose a map feature"
>
	<div class="px-3 pt-3 pb-2 text-xs font-semibold tracking-wide text-neutral-400 uppercase">
		Choose a feature
	</div>
	<div class="flex flex-col px-1.5 pb-1.5">
		{#each targets as target (`${target.kind}:${target.source}:${target.id}`)}
			<button
				type="button"
				aria-label={`Open ${target.kind} ${target.label}`}
				class="flex min-h-11 items-center gap-3 rounded-lg px-2.5 py-2 text-left hover:bg-neutral-800 focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-blue-400"
				onclick={() => onselect(target)}
			>
				<span
					class="flex size-8 shrink-0 items-center justify-center rounded-full bg-neutral-800 text-[10px] font-bold tracking-wide text-neutral-200 uppercase"
				>
					{target.kind.slice(0, 2)}
				</span>
				<span class="min-w-0">
					<span class="block truncate text-sm font-medium text-white">{target.label}</span>
					<span class="block truncate text-xs text-neutral-400">
						{target.subtitle ?? target.source.replace('_', ' ')}
					</span>
				</span>
			</button>
		{/each}
	</div>
</div>

<style>
	:global(.chooser-backdrop) {
		position: absolute;
		inset: 0;
		z-index: 60;
		background: transparent;
	}

	.chooser-panel {
		position: absolute;
		z-index: 81;
		left: var(--chooser-left);
		top: var(--chooser-top);
		width: min(18rem, calc(100% - 1rem));
		max-height: min(22rem, calc(100% - 1rem));
		overflow-y: auto;
		border: 1px solid rgb(64 64 64);
		border-radius: 0.75rem;
		background: rgb(10 10 10 / 0.96);
		box-shadow: 0 20px 45px rgb(0 0 0 / 0.45);
		backdrop-filter: blur(12px);
	}

	@media (max-width: 639px) {
		.chooser-panel {
			position: fixed;
			left: 0.5rem;
			right: 0.5rem;
			top: auto;
			bottom: calc(4.5rem + env(safe-area-inset-bottom));
			width: auto;
			max-height: min(45vh, 22rem);
		}

		:global(.chooser-backdrop) {
			position: fixed;
			background: rgb(0 0 0 / 0.22);
		}
	}

	@media (prefers-reduced-motion: no-preference) {
		.chooser-panel {
			animation: chooser-in 120ms ease-out;
		}
	}

	@keyframes chooser-in {
		from {
			opacity: 0;
			transform: translateY(5px) scale(0.98);
		}
	}
</style>
