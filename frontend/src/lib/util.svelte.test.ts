import { flushSync } from 'svelte';

import { createThrottled } from '#lib/util.svelte.js';

import { expect, test, vi } from 'vitest';

test('throttled bindings keep input immediate and cancel pending updates on disposal', () => {
	vi.useFakeTimers();
	let destroy = () => {};
	try {
		let search!: ReturnType<typeof createThrottled<string>>;
		destroy = $effect.root(() => {
			search = createThrottled('', 150);
		});
		flushSync();
		search.set('a');
		expect(search.get()).toBe('a');
		expect(search.current).toBe('a');
		search.set('ab');
		expect(search.get()).toBe('ab');
		expect(search.current).toBe('a');
		vi.advanceTimersByTime(150);
		expect(search.current).toBe('ab');
		search.set('');
		expect(search.get()).toBe('');
		destroy();
		vi.runAllTimers();
		expect(search.current).toBe('ab');
	} finally {
		destroy();
		vi.useRealTimers();
	}
});
