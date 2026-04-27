<script lang="ts">
	import { page } from '$app/state';

	import type { Source } from '$lib/client';
	import { source_info } from '$lib/resources/index.svelte';

	interface Props {
		sources: Source[];
	}

	let { sources = $bindable() }: Props = $props();
</script>

<div class="flex flex-col gap-2">
	<div class="text-sm font-semibold text-gray-700 dark:text-gray-300">Data Sources</div>
	<div class="flex flex-col gap-1">
		{#each page.data.selected_sources as source}
			{@const info = source_info[source]}
			<label class="grid grid-cols-[1fr_auto] items-center gap-2">
				<div class="flex items-center gap-2">
					<img src={info.icon} alt="" class="size-6 rounded-sm object-contain" />
					<span class="text-small">{info.name}</span>
				</div>
				<input
					type="checkbox"
					checked={sources.includes(source)}
					onchange={(e) => {
						if (e.currentTarget.checked) {
							sources = [...sources, source];
						} else {
							sources = sources.filter((s) => s !== source);
						}
					}}
				/>
			</label>
		{/each}
	</div>
</div>
