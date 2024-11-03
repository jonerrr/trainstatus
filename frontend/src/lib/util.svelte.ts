import { browser } from '$app/environment';

// from https://www.geeksforgeeks.org/haversine-formula-to-find-distance-between-two-points-on-a-sphere/
export function haversine(lat1: number, lon1: number, lat2: number, lon2: number) {
	// distance between latitudes and longitudes
	const dLat = ((lat2 - lat1) * Math.PI) / 180.0;
	const dLon = ((lon2 - lon1) * Math.PI) / 180.0;

	// convert to radians
	lat1 = (lat1 * Math.PI) / 180.0;
	lat2 = (lat2 * Math.PI) / 180.0;

	// apply formula
	const a =
		Math.pow(Math.sin(dLat / 2), 2) +
		Math.pow(Math.sin(dLon / 2), 2) * Math.cos(lat1) * Math.cos(lat2);
	const rad = 6371;
	const c = 2 * Math.asin(Math.sqrt(a));
	return rad * c;
}

export interface PersistedRune<T> {
	value: T;
	// key: string;
	reset: () => void;
}

export function persisted_rune<T>(key: string, init_value: T) {
	let storedValue: T;

	try {
		const item = localStorage.getItem(key);
		storedValue = item ? JSON.parse(item) : init_value;
	} catch (e) {
		// localStorage won't be defined so this will always throw on init load
		if (browser) console.error(e);
		storedValue = init_value;
	}

	let state = $state(storedValue);

	function updateStorage(value: T) {
		try {
			localStorage.setItem(key, JSON.stringify(value));
		} catch (e) {
			console.error(e);
		}
	}

	// listen for changes in other tabs
	if (browser) {
		window.addEventListener('storage', (event) => {
			if (event.key === key && event.storageArea === localStorage) {
				try {
					const newValue = event.newValue ? JSON.parse(event.newValue) : init_value;
					state = newValue;
				} catch (e) {
					console.error(e);
				}
			}
		});
	}

	// this allows it to update without being in a component
	$effect.root(() => {
		$effect(() => {
			updateStorage(state);
		});

		return () => {};
	});

	return {
		get value() {
			return state;
		},
		// get key() {
		// 	return key;
		// },
		set value(newValue: T) {
			state = newValue;
		},
		reset() {
			state = init_value;
		}
	};
}

export const stop_pins_rune = persisted_rune<number[]>('stop_pins', [106, 400086]);
export const route_pins_rune = persisted_rune<string[]>('route_pins', ['4', 'M15']);
export const trip_pins_rune = persisted_rune<string[]>('trip_pins', []);
