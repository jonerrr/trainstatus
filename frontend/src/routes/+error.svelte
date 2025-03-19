<script lang="ts">
	import { page } from '$app/state';
</script>

<div class="flex flex-col items-center justify-center rounded bg-neutral-950 p-4 backdrop-blur-xl">
	<h2 class="text-3xl font-semibold text-red-400">{page.status}: {page.error!.message}</h2>
	<a href="/" class="text-lg text-blue-400 underline">Go back home</a>
</div>
