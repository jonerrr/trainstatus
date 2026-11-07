import { createSubscriber } from 'svelte/reactivity';

import { browser } from '$app/env';

export type GeolocationStatus = 'loading' | 'active' | 'denied';

/**
 * Reactive browser geolocation watcher backed by `createSubscriber`.
 * Starts `watchPosition` while any reactive reader is mounted, cleans up via
 * `clearWatch` when the last reader unmounts, and retains the last known
 * position across route navigations, refreshes, and transient errors.
 */
export class Geolocation {
	#status = $state<GeolocationStatus>('loading');
	#position = $state.raw<GeolocationPosition>();
	#watch_id: number | undefined;
	#active = false;

	#subscribe = createSubscriber(() => {
		this.#active = true;
		this.#watch();
		return () => {
			this.#active = false;
			this.#stop();
		};
	});

	#watch(): void {
		if (!browser) return;
		if (!navigator.geolocation) {
			this.#status = 'denied';
			return;
		}
		this.#stop();
		this.#status = 'loading';
		this.#watch_id = navigator.geolocation.watchPosition(
			(position) => {
				this.#position = position;
				this.#status = 'active';
			},
			(err) => {
				console.error('Error watching position:', err);
				this.#status = 'denied';
			},
			{
				maximumAge: 30_000
			}
		);
	}

	#stop(): void {
		if (this.#watch_id !== undefined) {
			navigator.geolocation?.clearWatch(this.#watch_id);
			this.#watch_id = undefined;
		}
	}

	get status(): GeolocationStatus {
		this.#subscribe();
		return this.#status;
	}

	get position(): GeolocationPosition | undefined {
		this.#subscribe();
		return this.#position;
	}

	resume(): void {
		if (!browser) return;
		if (!navigator.geolocation) {
			this.#status = 'denied';
			return;
		}
		this.#status = 'loading';
		if (this.#active) {
			this.#watch();
		}
	}
}

export const geolocation = new Geolocation();
