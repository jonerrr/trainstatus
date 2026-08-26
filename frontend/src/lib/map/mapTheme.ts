import maplibregl from 'maplibre-gl';

/**
 * Shared visual language for the map: palette, line/circle sizing ramps, and the
 * layer-ordering anchors. Route lines, stop beads and deck.gl vehicle icons all
 * pull from here so their proportions stay in sync when one of them is tweaked.
 */

/** Dark casing drawn under route lines and around vehicles. Darker than the
 * `#0e0e0e` dark-matter background so it reads as a cut-out at every zoom. */
export const CASING_RGB: [number, number, number] = [8, 9, 12];
export const CASING = '#08090c';

/** Vehicle body fill. Head cars are pushed to pure white so the front of a
 * consist is distinguishable from its trailing cars. */
export const BODY_RGB: [number, number, number] = [236, 239, 243];
export const BODY_HEAD_RGB: [number, number, number] = [255, 255, 255];

/** Stop bead fill, and the dimmer grey used for the much denser bus stops. */
export const STOP_FILL = '#ffffff';
export const BUS_STOP_FILL = '#aeb8c4';

/** Used when a route has no usable colour. */
export const FALLBACK_ROUTE_COLOR = '#8b95a1';

/**
 * Route line colour. Colours are normalised to canonical `#RRGGBB` at ingest
 * (backend `RouteStore::save_all`), so the tile `color` property is used as-is;
 * the coalesce only guards the rare route that carries no colour at all.
 */
export const ROUTE_COLOR: maplibregl.ExpressionSpecification = [
	'coalesce',
	['get', 'color'],
	FALLBACK_ROUTE_COLOR
];

/** DOM-side counterpart of {@link ROUTE_COLOR}, for tooltips and route pills. */
export function normalizeRouteColor(color: string | null | undefined): string {
	return color?.trim() || FALLBACK_ROUTE_COLOR;
}

/**
 * Base route line width. This ramp is deliberately *constant* with respect to
 * hover: widening the line the cursor is currently over changes MapLibre's
 * hit-test on the next mousemove, which toggles the highlight, which resizes the
 * line again. Hover emphasis lives in a separate, non-interactive layer instead.
 */
export const ROUTE_WIDTH: maplibregl.ExpressionSpecification = [
	'interpolate',
	['linear'],
	['zoom'],
	10,
	1,
	15,
	3,
	18,
	6,
	20,
	10
];

/** Dark stroke under {@link ROUTE_WIDTH}, roughly 2px wider at every zoom. */
export const ROUTE_CASING_WIDTH: maplibregl.ExpressionSpecification = [
	'interpolate',
	['linear'],
	['zoom'],
	10,
	2.2,
	15,
	5,
	18,
	9,
	20,
	14
];

/** Width of the hover/selection overlay layer. */
export const ROUTE_HIGHLIGHT_WIDTH: maplibregl.ExpressionSpecification = [
	'interpolate',
	['linear'],
	['zoom'],
	10,
	2.5,
	15,
	7,
	18,
	14,
	20,
	20
];

/** Opacity applied to routes that are *not* the active one while something is hovered. */
export const ROUTE_DIMMED_OPACITY = 0.28;

/**
 * Width of the transparent layer that actually receives route hover and clicks.
 * Roughly 4-5x {@link ROUTE_WIDTH}: the drawn line is only 3px at z15, which is a
 * miserable mouse target. MapLibre still returns features from a layer painted at
 * zero opacity, and it buffers line hit-tests by `line-width`, so a fat invisible
 * copy widens the target without changing anything on screen.
 *
 * Like {@link ROUTE_WIDTH} this must stay independent of hover state — if the
 * grab area grew under the cursor it would reintroduce the highlight flicker.
 */
export const ROUTE_HIT_WIDTH: maplibregl.ExpressionSpecification = [
	'interpolate',
	['linear'],
	['zoom'],
	10,
	8,
	14,
	12,
	16,
	16,
	18,
	20,
	20,
	24
];

/**
 * Subway station bead. Sized so its diameter stays comfortably wider than
 * {@link ROUTE_WIDTH} at the same zoom, which is what makes the station read as
 * a bead threaded onto the line rather than a dot sitting beside it.
 */
export const STOP_BEAD_RADIUS: maplibregl.ExpressionSpecification = [
	'interpolate',
	['linear'],
	['zoom'],
	12,
	1.6,
	15,
	2.6,
	18,
	5,
	20,
	8
];

export const STOP_BEAD_STROKE: maplibregl.ExpressionSpecification = [
	'interpolate',
	['linear'],
	['zoom'],
	13,
	0.6,
	17,
	1.6,
	20,
	2.4
];

export const BUS_STOP_RADIUS: maplibregl.ExpressionSpecification = [
	'interpolate',
	['linear'],
	['zoom'],
	15,
	1.4,
	18,
	3.2,
	20,
	5
];

/**
 * Grab radii for the transparent stop hit layers. Kept deliberately modest:
 * stops outrank routes in {@link import('./hover.svelte').MapHover}, so an
 * over-large stop target would make the route line hard to grab near a station.
 *
 * Each hit layer must mirror its visible layer's filter *and* minzoom — a hit
 * area over a stop that isn't drawn yet is a click target the user cannot see.
 */
export const STOP_HIT_RADIUS: maplibregl.ExpressionSpecification = [
	'interpolate',
	['linear'],
	['zoom'],
	12,
	7,
	15,
	9,
	18,
	12,
	20,
	14
];

export const BUS_STOP_HIT_RADIUS: maplibregl.ExpressionSpecification = [
	'interpolate',
	['linear'],
	['zoom'],
	15,
	7,
	18,
	10,
	20,
	12
];

/**
 * `beforeId` anchors, so layer stacking is independent of the order components
 * happen to mount in. Toggling a layer group off and on re-adds it in the right
 * slot instead of on top of everything.
 *
 * These are real layers in geo/styles/dark-matter.json, listed here in ascending
 * z-order: `waterway_label` (66) is the bottom of the basemap label stack,
 * `watername_ocean` (67) sits just above it, `place_hamlet` (71) above that.
 * Anchoring to existing layers avoids having to mount invisible placeholders.
 */
export const SLOT = {
	/** Above roads and buildings, below every basemap label. */
	routes: 'waterway_label',
	/** Above the route lines. */
	stops: 'watername_ocean',
	/** Above the stop circles. */
	stopLabels: 'place_hamlet'
} as const;

/** Label font stack — the one the served dark-matter style actually ships glyphs for. */
export const LABEL_FONT = [
	'Montserrat Regular',
	'Open Sans Regular',
	'Noto Sans Regular',
	'HanWangHeiLight Regular',
	'NanumBarunGothic Regular'
];

export const SUBWAY_SOURCE_FILTER: maplibregl.ExpressionSpecification = [
	'==',
	['get', 'source'],
	'mta_subway'
];

export const BUS_SOURCE_FILTER: maplibregl.ExpressionSpecification = [
	'in',
	['get', 'source'],
	['literal', ['mta_bus', 'njt_bus']]
];
