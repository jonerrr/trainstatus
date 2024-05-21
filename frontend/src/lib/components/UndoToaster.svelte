<script lang="ts" context="module">
	export type ToastData = {
		title: string;
		description: string;
		// color: string;
		// store: Writable<string[]>;
		item: string;
		item_id: string;
	};

	const {
		elements: { content, title, description, close },
		helpers,
		states: { toasts },
		actions: { portal }
	} = createToaster<ToastData>();

	export const addToast = helpers.addToast;
</script>

<script lang="ts">
	import { createToaster, melt } from '@melt-ui/svelte';
	import { pinned_stops } from '$lib/stores';
</script>

<div use:portal>
	{#each $toasts as { id, data } (id)}
		<div use:melt={$content(id)}>
			<div>
				<div>
					<h3 use:melt={$title(id)}>
						{data.item} unpinned
						<!-- <span style:color={data.color} /> -->
					</h3>
					<div use:melt={$description(id)}>
						<button
							on:click={() => {
								pinned_stops.update((items) => [...items, data.item_id]);
							}}
						>
							Undo
						</button>
					</div>
				</div>
				<button use:melt={$close(id)} aria-label="Close notification"> X </button>
			</div>
		</div>
	{/each}
</div>
