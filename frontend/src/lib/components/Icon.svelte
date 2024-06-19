<script lang="ts">
	import { pushState } from '$app/navigation';
	import icons from '$lib/icons';

	export let name: string;
	export let width = '1rem';
	export let height = '1rem';
	// link to route alert, false for route alerts so you don't go to the same dialog
	export let link: boolean = true;

	let icon = icons.find((i) => i.name === name)!;
</script>

<!-- Icon component only works with complete: false svgs -->

<!-- TODO: when clicking on icon, go to stop list -->
<button
	on:click={() => {
		if (link) pushState('', { dialog_open: true, dialog_id: name, dialog_type: 'route_alert' });
	}}
	class="appearance-none"
>
	<svg class={$$props.class} {width} {height} viewBox="0 0 90 90">
		{@html icon.svg}
	</svg>
</button>
