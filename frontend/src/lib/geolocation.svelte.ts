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

	#subscribe = createSubscriber(() => {
		this.#watch();
		return () => this.#stop();
	});

	#watch(): void {
		if (!browser || !navigator.geolocation) return;
		this.#stop();
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
		if (!browser || !navigator.geolocation) return;
		this.#status = 'loading';
		this.#watch();
	}
}

export const geolocation = new Geolocation();
