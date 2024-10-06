export interface Trip<T extends TripData> {
	id: string;
	mta_id: string;
	route_id: string;
	status: TripStatus;
	stop_id?: number;
	vehicle_id: string;
	direction: TripDirection;
	data: T;
	created_at: Date;
	updated_at: Date;
}

type TripData = TrainTripData | BusTripData;

export interface TrainTripData {
	express: boolean;
	assigned: boolean;
}

export interface BusTripData {
	lat?: number;
	lon?: number;
	bearing?: number;
	passengers?: number;
	capacity?: number;
	deviation?: number;
}

export enum TripStatus {
	None = 'none',
	// train statuses
	Incoming = 'incoming',
	AtStop = 'at_stop',
	InTransitTo = 'in_transit_to',
	// bus statuses
	Spooking = 'spooking',
	Layover = 'layover',
	NoProgress = 'no_progress'
}

export enum TripDirection {
	North = 1,
	South = 0
}

export function createTrips() {
	// let trips: Trip<TripData>[] = $state([]);
	let trips = $state(new Map<string, Trip<TripData>>());

	function update() {
		fetch('/api/trips')
			.then((res) => res.json())
			.then(
				(data) =>
					// convert dates from strings to Date objects and put into map
					(trips = new Map(
						data.map((trip: Trip<TripData>) => [
							trip.id,
							{
								...trip,
								created_at: new Date(trip.created_at),
								updated_at: new Date(trip.updated_at)
							}
						])
					))
				// (trips = data.map((trip: Trip<TripData>) => ({
				// 	...trip,
				// 	created_at: new Date(trip.created_at),
				// 	updated_at: new Date(trip.updated_at)
				// })))
			);
	}

	return {
		update,
		get trips() {
			return trips;
		}
	};
}

export const trips = createTrips();
