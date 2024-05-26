<script lang="ts">
	import type { Trip } from '$lib/api';
	import List from '$lib/components/List.svelte';

	export let trips: Trip[];
</script>

<List loading={false} class="h-64">
	<!-- <div slot="header" class="flex self-center mb-2 w-full justify-between text-indigo-300">
		Trips
	</div> -->

	{#each trips as trip (trip.id)}
		<div
			class="border-neutral-600 bg-neutral-700 rounded border shadow-2xl my-1 hover:bg-neutral-900 px-1"
		>
			<div class="flex gap-2">
				<div class="flex-grow">asdads</div>
				<div>
					{trip.eta}
				</div>
			</div>
		</div>
	{/each}
</List>
