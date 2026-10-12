import type { ServiceTrip, Source } from '#lib/client/index.js';

import type { ScalePoint, ScaleTime } from 'd3-scale';
import { getLayerCakeContext } from 'layercake';

export type ChartPoint = {
	stop_id: string;
	stop_key: string;
	stop_name: string;
	time: Date;
};

export type ChartSeries = {
	source: Source;
	route_id: string;
	trip: ServiceTrip;
	points: ChartPoint[];
};

/** Read `cake.xScale` and the other fields in place. Copying them drops reactivity. */
export function chart_context() {
	return getLayerCakeContext<
		{ x: ScaleTime<Date, number>; y: ScalePoint<string> },
		ChartSeries[]
	>();
}
