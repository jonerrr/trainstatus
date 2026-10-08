<script lang="ts">
	import { onMount, type Snippet } from 'svelte';

	import { X } from '@lucide/svelte';

	import type { MapTarget, ScreenPoint } from './interactions';

	let {
		targets,
		point,
		viewportWidth,
		viewportHeight,
		onselect,
		ondismiss,
		children
	}: {
		targets: MapTarget[];
		point: ScreenPoint;
		viewportWidth: number;
		viewportHeight: number;
		onselect: (target: MapTarget) => void;
		ondismiss: () => void;
		children: Snippet<[MapTarget]>;
	} = $props();
	let panel = $state<HTMLDivElement>();
	let width = $state(288);
	let height = $state(0);
	let previous: HTMLElement | null = null;
	const left = $derived(Math.max(8, Math.min(point.x, viewportWidth - width - 8)));
	const top = $derived(Math.max(8, Math.min(point.y, viewportHeight - height - 8)));
	onMount(() => {
		previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
		panel?.querySelector<HTMLButtonElement>('[data-map-target]')?.focus();
	});
	function dismiss() {
		ondismiss();
		previous?.focus({ preventScroll: true });
	}
	function outside(event: PointerEvent) {
		if (!(event.target instanceof Node) || panel?.contains(event.target)) return;
		const focusable =
			event.target instanceof Element &&
			event.target.closest(
				'a[href], button, input, select, textarea, [tabindex]:not([tabindex="-1"])'
			);
		ondismiss();
		// The click's own focus move happens after pointerdown. Put the trigger
		// back once that settles, unless the click landed on another control.
		if (!focusable) setTimeout(() => previous?.focus({ preventScroll: true }));
	}
</script>

<svelte:window
	onpointerdown={outside}
	onkeydown={(event) => {
		if (event.key === 'Escape') {
			event.preventDefault();
			event.stopPropagation();
			dismiss();
		}
	}}
/>
<div
	bind:this={panel}
	bind:clientWidth={width}
	bind:clientHeight={height}
	class="chooser-panel"
	style:--panel-left={`${left}px`}
	style:--panel-top={`${top}px`}
	role="dialog"
	aria-label="Choose a map feature"
	data-map-control
>
	<header class="flex items-center justify-between border-b border-neutral-800 px-3 py-2">
		<h2 class="text-xs font-medium text-neutral-400">Choose a feature</h2>
		<button
			type="button"
			aria-label="Close feature chooser"
			onclick={dismiss}
			class="rounded-md p-1 text-neutral-300 hover:bg-neutral-800"><X size={16} /></button
		>
	</header>
	<div class="flex flex-col gap-1 p-1.5">
		{#each targets as target (`${target.kind}:${target.source}:${target.id}`)}
			<button
				type="button"
				class="min-h-11 rounded-lg p-2.5 hover:bg-neutral-800 focus-visible:outline-2 focus-visible:outline-blue-400"
				data-map-target
				onclick={() => onselect(target)}
			>
				{@render children(target)}
			</button>
		{/each}
	</div>
</div>

<style>
	.chooser-panel {
		position: absolute;
		left: var(--panel-left);
		top: var(--panel-top);
		z-index: 40;
		width: min(20rem, calc(100% - 1rem));
		max-height: calc(100% - 1rem);
		overflow: auto;
		border: 1px solid #404040;
		border-radius: 0.75rem;
		background: #0a0a0af5;
		box-shadow: 0 8px 24px #0005;
	}
	@media (max-width: 639px) {
		.chooser-panel {
			top: auto;
			left: 0.5rem;
			right: 0.5rem;
			bottom: 0.5rem;
			width: auto;
			max-height: 40dvh;
		}
	}
</style>
