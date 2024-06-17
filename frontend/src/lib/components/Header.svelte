<script lang="ts">
	import { offline } from '$lib/stores';
</script>

<header class="text-4xl p-2 font-bold text-indigo-400 flex justify-between">
	<div
		class="font-bold"
		style="background: linear-gradient(45deg, #c026d3, #4338ca); -webkit-background-clip: text; -webkit-text-fill-color: transparent;"
	>
		Trainstat.us
	</div>
	{#if $offline}
		<div class="text-red-500 text-xs">Offline</div>
	{/if}
</header>
