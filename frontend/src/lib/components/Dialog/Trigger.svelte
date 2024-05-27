<script lang="ts">
	import { melt } from '@melt-ui/svelte';
	import { dialogRegistry, type DialogName } from '.';

	export let name: DialogName;
	export let offsetHeight = 0;

	const {
		elements: { trigger }
	} = dialogRegistry.get(name);
</script>

<button bind:offsetHeight class="w-full flex justify-between items-center py-1" use:melt={$trigger}>
	<slot>
		Open {name} Dialog
	</slot>
</button>
