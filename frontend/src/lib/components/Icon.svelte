<script lang="ts">
	import icons from '$lib/icons';

	export let name: string;
	export let width = '1rem';
	export let height = '1rem';
	export let opacity: 'none' | 'half' | 'full' = 'none';

	let icon = icons.find((i) => i.name === name)!;
</script>

<!-- {#if icon.complete_svg}
	{@html icon.svg}
{:else} -->
<!-- TODO: when clicking on icon, go to stop list -->
<svg class={$$props.class} {width} {height} viewBox="0 0 90 90">
	{@html icon.svg}
</svg>
<!-- {/if} -->
