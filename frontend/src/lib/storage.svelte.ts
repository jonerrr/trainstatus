/* eslint-disable svelte/prefer-svelte-reactivity -- this registry is listener
   bookkeeping; views track each instance's native value, not registry membership. */
import { createSubscriber } from 'svelte/reactivity';

import { browser } from '$app/env';

interface StoredValue {
	refresh(): void;
}

// Weak references let temporary list/tab preferences disappear with their component.
const instances = new Map<string, Set<WeakRef<StoredValue>>>();

function refresh(key: string | null) {
	for (const [name, values] of instances) {
		if (key !== null && name !== key) continue;
		for (const ref of values) {
			const value = ref.deref();
			if (value) value.refresh();
			else values.delete(ref);
		}
		if (!values.size) instances.delete(name);
	}
}

// createSubscriber ties the single external listener to mounted readers, including module-level preferences.
const subscribe = createSubscriber(() => {
	if (!browser) return;
	refresh(null);
	const changed = (event: StorageEvent) => {
		if (event.storageArea === localStorage) refresh(event.key);
	};
	window.addEventListener('storage', changed);
	return () => window.removeEventListener('storage', changed);
});

/** Cached preferences. Replace `current` to change and persist a value. */
export class LocalStorage<T> {
	#value = $state.raw<T>();

	constructor(
		readonly key: string,
		readonly initial: T
	) {
		this.refresh();
		if (!browser) return;
		let values = instances.get(key);
		if (!values) instances.set(key, (values = new Set()));
		values.add(new WeakRef(this));
	}

	refresh() {
		let next = this.initial;
		if (browser) {
			try {
				const saved = localStorage.getItem(this.key);
				if (saved !== null) next = JSON.parse(saved) as T;
			} catch {
				// An inaccessible or malformed saved value uses the in-memory default.
			}
		}
		if (JSON.stringify(next) !== JSON.stringify(this.#value)) this.#value = next;
	}

	get current(): T {
		subscribe();
		return this.#value!;
	}

	set current(value: T) {
		this.#value = value;
		if (!browser) return;
		try {
			localStorage.setItem(this.key, JSON.stringify(value));
			refresh(this.key);
		} catch {
			// Preferences still work in memory when persistence is unavailable.
		}
	}
}
