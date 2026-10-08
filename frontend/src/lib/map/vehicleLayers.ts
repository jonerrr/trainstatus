import { IconLayer } from '@deck.gl/layers';

import type { MapFeatureKey } from './interactions';
import { BODY_HEAD_RGB, BODY_RGB, CASING_RGB } from './mapTheme';
import { normalizeBearingForIcon, type ActiveVehicle } from './trajectoryArrow';
import {
	shapeForIconKey,
	VEHICLE_ICON_ATLAS,
	VEHICLE_ICON_MAPPING,
	type VehicleIconRole
} from './vehicleIcons';

const RAIL_PUCK_PX = 22;
const BUS_PUCK_PX = 18;
const DIMMED = 0.65;
const HOVER_SCALE = 1.12;

export function isRailVehicle(vehicle: ActiveVehicle) {
	return vehicle.iconKey === 'rail_head' || vehicle.iconKey === 'rail_car';
}

export function vehicleLayers(
	partitions: { detail: ActiveVehicle[]; puck: ActiveVehicle[] },
	selectedTrip: MapFeatureKey | null,
	frameVersion: number,
	dataVersion: number
) {
	const selectedKey = selectedTrip ? `${selectedTrip.source}:${selectedTrip.id}` : null;

	function isSelected(vehicle: ActiveVehicle) {
		return selectedTrip?.source === vehicle.source && selectedTrip.id === vehicle.tripId;
	}

	function alphaFor(vehicle: ActiveVehicle, base: number) {
		const dim = selectedKey && !isSelected(vehicle) ? DIMMED : 1;
		return Math.round(base * dim);
	}

	function bodyColor(vehicle: ActiveVehicle): [number, number, number, number] {
		// Buses keep a light body too: filling them with the route color made them
		// disappear into the identically colored route line they sit on.
		const rgb = vehicle.isHead ? BODY_HEAD_RGB : BODY_RGB;
		return [rgb[0], rgb[1], rgb[2], alphaFor(vehicle, 245)];
	}

	function ringColor(vehicle: ActiveVehicle): [number, number, number, number] {
		return [vehicle.color[0], vehicle.color[1], vehicle.color[2], alphaFor(vehicle, 255)];
	}

	function casingColor(vehicle: ActiveVehicle): [number, number, number, number] {
		return [CASING_RGB[0], CASING_RGB[1], CASING_RGB[2], alphaFor(vehicle, 235)];
	}

	function colorFor(
		vehicle: ActiveVehicle,
		role: VehicleIconRole
	): [number, number, number, number] {
		if (role === 'casing') return casingColor(vehicle);
		if (role === 'ring') return ringColor(vehicle);
		return bodyColor(vehicle);
	}

	/**
	 * The casing / ring / body passes are three concentric silhouettes of the same
	 * shape drawn at the same size and anchor, so the outline thickness is a fixed
	 * proportion of the icon. The previous approach — one shape at 1.18x under a
	 * copy at 0.94x — collapsed to a 1px fringe as soon as both hit the min-pixel
	 * clamp, which is a large part of why the markers looked muddy.
	 */
	function iconLayers(
		idPrefix: string,
		data: ActiveVehicle[],
		getSize: (vehicle: ActiveVehicle) => number,
		sizeUnits: 'meters' | 'pixels',
		puckShape: boolean
	) {
		return (['casing', 'ring', 'body'] as const).map(
			(role) =>
				new IconLayer<ActiveVehicle>({
					id: `${idPrefix}-${role}`,
					data,
					iconAtlas: VEHICLE_ICON_ATLAS,
					iconMapping: VEHICLE_ICON_MAPPING,
					getIcon: (vehicle) => `${puckShape ? 'puck' : shapeForIconKey(vehicle.iconKey)}_${role}`,
					getPosition: (vehicle) => vehicle.position,
					getColor: (vehicle) => colorFor(vehicle, role),
					getAngle: (vehicle) => normalizeBearingForIcon(vehicle.bearing),
					getSize,
					sizeUnits,
					billboard: false,
					alphaCutoff: 0,
					sizeMinPixels: sizeUnits === 'meters' ? 3 : 0,
					// Only the topmost pass is pickable, so a hover produces one hit
					// rather than three stacked ones.
					pickable: role === 'body',
					updateTriggers: {
						getPosition: frameVersion,
						getAngle: frameVersion,
						getIcon: dataVersion,
						getColor: [dataVersion, selectedKey],
						getSize: [dataVersion, selectedKey]
					}
				})
		);
	}

	const { detail, puck } = partitions;
	const layers: IconLayer<ActiveVehicle>[] = [];

	if (puck.length > 0) {
		layers.push(
			...iconLayers(
				'trip-markers-puck',
				puck,
				(vehicle) => {
					const base = isRailVehicle(vehicle) ? RAIL_PUCK_PX : BUS_PUCK_PX;
					return isSelected(vehicle) ? base * HOVER_SCALE : base;
				},
				'pixels',
				true
			)
		);
	}

	if (detail.length > 0) {
		layers.push(
			...iconLayers(
				'trip-markers-detail',
				detail,
				(vehicle) => (isSelected(vehicle) ? vehicle.lengthM * HOVER_SCALE : vehicle.lengthM),
				'meters',
				false
			)
		);
	}

	return layers;
}
