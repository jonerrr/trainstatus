import type { ServiceTrip, Source } from '#lib/client/index.js';

import type { ScalePoint, ScaleTime } from 'd3-scale';
import { getLayerCakeContext } from 'layercake';

export type ChartPoint = {
	stopId: string;
	stopKey: string;
	stopName: string;
	time: Date;
};

export type ChartSeries = {
	source: Source;
	routeId: string;
	trip: ServiceTrip;
	points: ChartPoint[];
};

/** Read `cake.xScale` and the other fields in place. Copying them drops reactivity. */
export function chartContext() {
	return getLayerCakeContext<
		{ x: ScaleTime<Date, number>; y: ScalePoint<string> },
		ChartSeries[]
	>();
}
