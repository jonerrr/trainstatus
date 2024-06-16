<script lang="ts">
	import icons from '$lib/icons';

	export let name: string;
	export let width = '1rem';
	export let height = '1rem';

	let icon = icons.find((i) => i.name === name)!;
</script>

<!-- Icon component only works with complete: false svgs -->

<!-- TODO: when clicking on icon, go to stop list -->
<svg class={$$props.class} {width} {height} viewBox="0 0 90 90">
	{@html icon.svg}
</svg>
