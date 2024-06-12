// export async function init_bus_data() {
// 	try {
// 		const [busResponse] = await Promise.all([fetch('/api/bus')]);

// 		const [buses] = await Promise.all([
// 			busResponse.json().then((data: Bus[]) => {
// 				return data.map((b: Bus) => {
// 					return {
// 						...b,
// 						created_at: new Date(b.created_at)
// 					};
// 				});
// 			})
// 		]);

// 		return buses;
// 	} catch (error) {
// 		console.error(error);
// 	}
// }

export interface BusStop {
	id: number;
	name: string;
	direction: string;
	lat: number;
	lon: number;
}
