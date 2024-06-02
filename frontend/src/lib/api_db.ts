// check if +layout.ts and invalidate work with dexie
import Dexie from 'dexie';
import { writable } from 'svelte/store';
import { db, type Stop, type StopTime, type Trip } from '$lib/db';

// const dateTimeReviver = function (key, value) {
// 	var a;
// 	if (typeof value === 'string') {
// 		a = Date.parse(value);
// 		if (a) {
// 			return new Date(a);
// 		}
// 	}
// 	return value;
// };

export const stop_store = writable<Stop[]>([]);
export const trip_store = writable<Trip[]>([]);
export const stop_time_store = writable<StopTime[]>([]);

export async function init_stops() {
	try {
		// TODO: promise all trips and stops
		// TODO: add check for if stops are already in db
		const Start = new Date().getTime();
		console.log('DB: fetching stops');
		const stops: Stop[] = await (await fetch('/api/stops')).json();
		// await db.stop.bulkPut(stops);
		stop_store.set(stops);

		console.log('DB: fetching trips');
		const trips_res = await fetch(`/api/trips?times=false`);

		const trips: Trip[] = (await trips_res.json()).map((t: Trip) => {
			return {
				...t,
				created_at: new Date(t.created_at)
			};
		});
		trip_store.set(trips);

		// insert to db, put will overwrite existing data
		// await db.trip.bulkPut(trips);
		// console.log('DB: inserted trips');

		const stop_times_res = await fetch('/api/arrivals');
		const stop_times: StopTime[] = (await stop_times_res.json()).map((st: StopTime) => {
			return {
				...st,
				arrival: new Date(st.arrival),
				departure: new Date(st.departure)
			};
		});
		stop_time_store.set(stop_times);
		// await db.stop_time.bulkPut(stop_times);
		// console.log('DB: inserted stop_times');
		const End = new Date().getTime();
		console.log('DB: time to fetch and insert data:', End - Start);
	} catch (e) {
		console.error(e);
	}
}
