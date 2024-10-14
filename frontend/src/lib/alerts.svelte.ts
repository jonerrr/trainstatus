import { SvelteMap } from 'svelte/reactivity';
import icons from './icons';

export interface Alert {
	id: string;
	alert_type: string;
	header_html: string;
	description_html?: string;
	start_time: Date;
	end_time?: Date;
	updated_at: Date;
	created_at: Date;
	entities: Entity[];
}

export interface Entity {
	route_id: string;
	sort_order: number;
	stop_id?: string;
}

type Fetch = typeof fetch;

const train_regex = /(\[(.+?)\])/gm;

function parse_html(html: string) {
	return html.replaceAll(train_regex, (_match, _p1, p2) => {
		const icon = icons.find((t) => t.name === p2) ?? icons[icons.length - 1];
		if (icon.complete_svg) return icon.svg;
		else
			return `<svg xmlsn="http://www.w3.org/2000/svg" class="inline-block" width="1rem" height="1rem" viewBox="0 0 90 90" focusable="false"> ${icon.svg} </svg>`;
	});
}

export function createAlerts() {
	let alerts: Alert[] = $state([]);
	const alerts_by_route: SvelteMap<string, Alert[]> = $state(new SvelteMap());

	async function update(fetch: Fetch) {
		try {
			const data: Alert[] = await (await fetch(`/api/v1/alerts`)).json();

			alerts = data.map((alert) => ({
				...alert,
				header_html: parse_html(alert.header_html),
				description_html: alert.description_html ? parse_html(alert.description_html) : undefined,
				start_time: new Date(alert.start_time),
				end_time: alert.end_time ? new Date(alert.end_time) : undefined,
				updated_at: new Date(alert.updated_at),
				created_at: new Date(alert.created_at)
			}));
			alerts_by_route.clear();
			// alerts_by_route = new SvelteMap<string, Alert[]>();
			for (const alert of alerts) {
				for (const entity of alert.entities) {
					if (!alerts_by_route.has(entity.route_id)) {
						alerts_by_route.set(entity.route_id, []);
					}
					alerts_by_route.get(entity.route_id)!.push(alert);
				}
			}

			return false;
		} catch (e) {
			console.error(e);
			return true;
		}

		// fetch(
		// 	`/api/stop_times${routes.length ? `?bus_route_ids=${encodeURIComponent(routes.join(','))}` : ''}`
		// )
		// 	.then((res) => res.json())
		// 	.then(e
		// 		(data) =>
		// 			// convert dates from strings to Date objects
		// 			(stop_times = data.map((stop_time: StopTime) => ({
		// 				...stop_time,
		// 				arrival: new Date(stop_time.arrival),
		// 				departure: new Date(stop_time.departure)
		// 			})))
		// 	);
		// TODO: add error handling and set offline status
	}

	return {
		update,

		get alerts() {
			return alerts;
		},

		get alerts_by_route() {
			return alerts_by_route;
		}
	};
}

export const alerts = createAlerts();
