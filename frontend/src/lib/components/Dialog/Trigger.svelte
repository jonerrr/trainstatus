<script lang="ts">
	import { melt } from '@melt-ui/svelte';
	import { dialogRegistry, type DialogName } from '.';

	export let name: DialogName;

	const {
		elements: { trigger }
	} = dialogRegistry.get(name);
</script>

<button class="w-full flex justify-between items-center" use:melt={$trigger}>
	<slot>
		Open {name} Dialog
	</slot>
</button>
