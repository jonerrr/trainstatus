<script lang="ts">
	import { page } from '$app/state';
</script>

<div class="flex flex-col justify-center items-center bg-neutral-950 backdrop-blur-xl rounded p-4">
	<h2 class="text-red-400 text-3xl font-semibold">{page.status}: {page.error!.message}</h2>
	<a href="/" class="text-blue-400 text-lg underline">Go back home</a>
</div>
