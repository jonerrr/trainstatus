import type { Stop } from '@trainstatus/client';

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

// TODO: remove since not used i think
export function debounce<T extends (...args: never[]) => void>(func: T, wait: number = 75) {
	let timeout: ReturnType<typeof setTimeout> | null;
	return function (...args: Parameters<T>) {
		if (timeout) clearTimeout(timeout);
		timeout = setTimeout(() => {
			timeout = null;
			func(...args);
		}, wait);
	};
}

export const calculate_route_height = () => 54;

export function calculate_stop_height(item: Stop) {
	let height = 44; // stop name height (28px) + 16px padding

	if (item.data.source === 'mta_bus') {
		height += item.routes.length * 56;
	} else if (item.data.source === 'mta_subway') {
		// headsign height
		height += 24;
		// route arrivals height
		height += item.routes.length * 24;
	}
	// TODO: handle other sources

	return height;
}

// export function get_position(): Promise<GeolocationPosition> {
// 	return new Promise<GeolocationPosition>((res, rej) => {
// 		navigator.geolocation.getCurrentPosition(res, rej);
// 	});
// }

// interface ItemHeights {
// 	[key: string]: number;
// }

// export function list_max_height(init: number = 0) {
// 	const height = browser
// 		? document
// 				.querySelectorAll('.sub-height')
// 				.values()
// 				.reduce((acc, el) => {
// 					acc += (el as HTMLElement).offsetHeight;
// 					return acc;
// 				}, init)
// 		: 124 + init;
// 	return `max-h-[calc(100dvh-${height}px)]`;
// }
// TODO: remove if not used. Was used to store virtual list item heights.
// need to test if its faster to calculate heights on the fly or store them in a map like this.
// export const item_heights = $state<ItemHeights>({});
