import type { Trip } from '#lib/client/index.js';

import type { ScalePoint, ScaleTime } from 'd3-scale';
import { getLayerCakeContext } from 'layercake';

export type ChartPoint = {
	stop_id: string;
	stop_key: string;
	stop_name: string;
	time: Date;
};

export type ChartSeries = {
	trip: Trip;
	points: ChartPoint[];
};

/** Read `cake.xScale` and the other fields in place. Copying them drops reactivity. */
export function chart_context() {
	return getLayerCakeContext<
		{ x: ScaleTime<Date, number>; y: ScalePoint<string> },
		ChartSeries[]
	>();
}
