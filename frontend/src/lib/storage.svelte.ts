/* eslint-disable svelte/prefer-svelte-reactivity -- this registry is listener
   bookkeeping; views track each instance's native value, not registry membership. */
import { browser } from '$app/env';

interface StoredValue {
	refresh(): void;
}

// Weak references let temporary list/tab preferences disappear with their component.
const instances = new Map<string, Set<WeakRef<StoredValue>>>();
let mounts = 0;

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

function storageChanged(event: StorageEvent) {
	if (event.storageArea === localStorage) refresh(event.key);
}

/** One cross-tab listener, owned by mounted application layouts/providers. */
export function syncLocalStorage() {
	if (!browser) return;
	if (mounts++ === 0) {
		refresh(null);
		window.addEventListener('storage', storageChanged);
	}
	return () => {
		if (--mounts === 0) window.removeEventListener('storage', storageChanged);
	};
}

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
