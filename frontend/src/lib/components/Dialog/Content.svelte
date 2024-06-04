<script lang="ts">
	import { createSync, melt } from '@melt-ui/svelte';
	import { dialogRegistry, type DialogName } from '.';
	import { page } from '$app/stores';
	// import { flyAndScale } from '$lib/utils';

	export let name: DialogName;

	const {
		elements: { portalled, title, content, description, close, overlay },
		states: { open }
	} = dialogRegistry.get(name);

	const sync = createSync({ open });
	// @ts-ignore I don't think this is an issue
	$: sync.open($page.state.dialogOpen === name, ($open) => {
		console.log('sync', $open, $page.state, name);
		// @ts-ignore
		if ($page.state.dialogOpen !== name) {
			dialogRegistry.shallow(name, $open);
		}
	});
</script>

<!-- TODO: prevent clicking on other dialog when closing -->
<!-- TODO: add fade transition -->

{#if $open}
	<div use:melt={$portalled}>
		<div use:melt={$overlay} class="fixed inset-0 z-40 bg-black/40" />
		<div
			class="fixed left-[50%] top-[50%] z-50 max-h-[75vh]
            max-w-[calc(100vw - 20px)] translate-x-[-50%] translate-y-[-50%] rounded bg-neutral-800 text-indigo-300
            p-6 shadow-lg overflow-auto"
			use:melt={$content}
		>
			<slot title={$title} description={$description} close={$close} />
		</div>
	</div>
{/if}
