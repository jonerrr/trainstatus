// check if +layout.ts and invalidate work with dexie
import Dexie from 'dexie';
import { db, type StopTime, type Trip } from '$lib/db';

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

export async function init_stops() {
	try {
		// TODO: add check for if stops are already in db
		const Start = new Date().getTime();
		console.log('DB: fetching trips');
		const trips_res = await fetch(`/api/trips`);

		// TODO: promise all trips and stops
		const trips: Trip[] = (await trips_res.json()).map((t: Trip) => {
			return {
				...t,
				created_at: new Date(t.created_at)
			};
		});

		// insert to db, put will overwrite existing data
		await db.trip.bulkPut(trips);
		console.log('DB: inserted trips');

		const stop_times_res = await fetch('/api/arrivals');
		const stop_times: StopTime[] = (await stop_times_res.json()).map((st: StopTime) => {
			return {
				...st,
				arrival: new Date(st.arrival),
				departure: new Date(st.departure)
			};
		});
		// console.log(stop_times);

		await db.stop_time.bulkPut(stop_times);

		console.log('DB: inserted times');

		// Your function or code block here
		const end = new Date().getTime();

		// get trips for stop id
		// example of what we need for showing first 2 etas of each route at a stop
		// const stops_test = await db.trip.where('stop_id').equals('250').toArray();
		// console.log(await db.stop_time.toArray());
		// const arrivals = await db.stop_time.where('stop_id').equals('250').toArray();

		// console.log(arrivals);
		console.log(end - Start);

		// await Promise.all(
		// stops_test.map(async (s) => {
		// 	await db.stop_time
		// })
		// );
		// console.log(stops_test);
	} catch (e) {
		console.error(e);
	}
}
