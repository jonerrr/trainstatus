// mostly from: https://github.com/Rich-Harris/local-storage-test/blob/main/src/lib/storage.svelte.ts (changed browser checks)
import { tick } from 'svelte';

import { browser } from '$app/environment';

export class LocalStorage<T> {
	#key: string;
	#version = $state(0);
	#listeners = 0;
	#value: T;

	#handler = (e: StorageEvent) => {
		if (e.storageArea !== localStorage) return;
		if (e.key !== this.#key) return;

		this.#version += 1;
	};

	constructor(key: string, initial: T) {
		this.#key = key;
		this.#value = initial;

		if (browser) {
			if (localStorage.getItem(key) === null) {
				localStorage.setItem(key, JSON.stringify(initial));
			}
		}
	}

	get current(): T {
		this.#version;

		const root: T = browser ? JSON.parse(localStorage.getItem(this.#key) as string) : this.#value;

		const proxies = new WeakMap();

		const proxy = <V>(value: V): V => {
			if (typeof value !== 'object' || value === null) {
				return value;
			}

			let p = proxies.get(value);

			if (!p) {
				p = new Proxy(value as object, {
					get: (target, property) => {
						this.#version;
						return proxy(Reflect.get(target, property));
					},
					set: (target, property, value) => {
						this.#version += 1;
						Reflect.set(target, property, value);

						if (browser) {
							localStorage.setItem(this.#key, JSON.stringify(root));
						}

						return true;
					}
				});

				proxies.set(value, p);
			}

			return p as V;
		};

		if ($effect.tracking()) {
			$effect(() => {
				if (this.#listeners === 0) {
					window.addEventListener('storage', this.#handler);
				}

				this.#listeners += 1;

				return () => {
					tick().then(() => {
						this.#listeners -= 1;
						if (this.#listeners === 0) {
							window.removeEventListener('storage', this.#handler);
						}
					});
				};
			});
		}

		return proxy(root);
	}

	set current(value: T) {
		if (browser) {
			localStorage.setItem(this.#key, JSON.stringify(value));
		}

		this.#version += 1;
	}
}
