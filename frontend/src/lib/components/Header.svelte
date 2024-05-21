<script lang="ts">
	import { offline } from '$lib/stores';
</script>

<header class="text-4xl p-2 font-bold text-indigo-400 flex">
	<span class="text-gradient">Trainstat</span>
	<span class="text-zinc-600">.</span><span class={$offline ? 'text-red-600' : 'text-blue-400'}
		>us</span
	>
</header>

<style lang="postcss">
	.text-gradient {
		/* background-image: linear-gradient(45deg, #f3ec78, #af4261);
		-webkit-background-clip: text;
		background-clip: text;
		color: transparent; */
		--bg-size: 400%;
		--color-one: #4338ca;
		--color-two: #a21caf;
		font-family: sans-serif;
		background: linear-gradient(90deg, var(--color-one), var(--color-two), var(--color-one)) 0 0 /
			var(--bg-size) 100%;
		color: transparent;
		background-clip: text;
	}

	@media (prefers-reduced-motion: no-preference) {
		.text-gradient {
			animation: move-bg 8s linear infinite;
		}
		@keyframes move-bg {
			to {
				background-position: var(--bg-size) 0;
			}
		}
	}
</style>
