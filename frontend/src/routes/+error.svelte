<script lang="ts">
	import { page } from '$app/state';
</script>

<div class="flex flex-col justify-center items-center">
	<h1 class="text-red-400 text-4xl">{page.status}: {page.error!.message}</h1>
	<a href="/" class="text-blue-400 underline">Go back home</a>
</div>
