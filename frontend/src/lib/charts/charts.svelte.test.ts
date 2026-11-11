import type { Source } from '#lib/client/index.js';

import '../../app.css';

import { page } from '#lib/test/page.svelte.js';
import {
	fixtureFetcher,
	fixtureTransitData,
	type TransitFixture
} from '#lib/test/transit-fixture.js';
import TransitProvider from '#lib/test/TransitProvider.svelte';
import { TransitData } from '#lib/transit/store.svelte.js';

import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { userEvent } from 'vitest/browser';

import ChartsPage from '../../routes/charts/+page.svelte';
import { chartFixture, chartNow } from './fixtures.js';

const clock = vi.hoisted(() => ({ ms: 0 }));

vi.mock('#lib/url_params.svelte.js', async (original) => ({
	...(await original<typeof import('#lib/url_params.svelte.js')>()),
	getCurrentTime: () => ({ value: undefined, ms: clock.ms }),
	open_modal: () => {}
}));

let fixture: TransitFixture = { catalogs: {}, services: {} };
let data: TransitData | undefined;

function setFixture(source: Source, direction: number, patterns?: string[][]) {
	clock.ms = chartNow;
	const built = chartFixture(source, direction, patterns);
	fixture = { catalogs: { [source]: built.catalog }, services: { [source]: built.service } };
	page.data = { selected_sources: [source] };
	return built;
}

afterEach(() => {
	data?.destroy();
	data = undefined;
});

async function renderChart(provided?: TransitData) {
	data = provided ?? fixtureTransitData(fixture);
	const { container } = await render(TransitProvider, {
		props: {
			data,
			component: ChartsPage as never,
			child_props: {}
		}
	});
	container.style.width = '1400px';
	container.style.height = '800px';
}

async function expectChartSize() {
	await expect
		.poll(
			() => document.querySelector('.layercake-layout-svg')?.getBoundingClientRect().height ?? 0
		)
		.toBeGreaterThan(400);
}

for (const source of ['mta_subway', 'mta_bus', 'njt_bus'] satisfies Source[]) {
	for (const direction of [0, 1]) {
		test(`${source} direction ${direction}: every rendered trip progresses through the stop axis`, async () => {
			const built = setFixture(source, direction);
			await renderChart();
			if (source !== 'mta_subway') {
				await userEvent.click(document.querySelector('[aria-label="Add routes"]')!);
				await userEvent.click(document.querySelector('[role="option"]')!);
			}
			if (direction === 1) await userEvent.click(document.querySelector('#southbound')!);
			await expect
				.poll(() => document.querySelectorAll('.path-line').length)
				.toBe(built.trips.length);
			await expectChartSize();
			for (const [index, path] of [...document.querySelectorAll('.path-line')].entries()) {
				const d = path.getAttribute('d')!;
				expect(d).not.toMatch(/NaN|undefined/);
				const points = [...d.matchAll(/[ML](-?[\d.]+),(-?[\d.]+)/g)].map((m) => [
					Number(m[1]),
					Number(m[2])
				]);
				expect(points.length).toBe(built.patterns[index].length);
				expect(
					points.every(([x, y]) => Number.isFinite(x) && Number.isFinite(y) && x >= 0 && y >= 0)
				).toBe(true);
				for (let i = 1; i < points.length; i++) {
					expect.soft(points[i][0]).toBeGreaterThan(points[i - 1][0]);
					expect.soft(points[i][1]).toBeGreaterThan(points[i - 1][1]);
				}
			}
			expect(document.querySelectorAll('.stop-name').length).toBe(5);
			await userEvent.click(document.querySelector('#stop_points')!);
			expect(document.querySelectorAll('.layercake-layout-svg circle').length).toBe(11);
		});

		test(`${source} direction ${direction}: loop visits render as separate axis rows`, async () => {
			setFixture(source, direction, [
				['a', 'b', 'a', 'c'],
				['a', 'c', 'b']
			]);
			await renderChart();
			if (source !== 'mta_subway') {
				await userEvent.click(document.querySelector('[aria-label="Add routes"]')!);
				await userEvent.click(document.querySelector('[role="option"]')!);
			}
			if (direction === 1) await userEvent.click(document.querySelector('#southbound')!);
			await expect.poll(() => document.querySelectorAll('.path-line').length).toBe(2);
			await expectChartSize();
			for (const path of document.querySelectorAll('.path-line')) {
				const points = [...path.getAttribute('d')!.matchAll(/[ML](-?[\d.]+),(-?[\d.]+)/g)].map(
					(m) => [Number(m[1]), Number(m[2])]
				);
				for (let i = 1; i < points.length; i++) {
					expect(points[i][0]).toBeGreaterThan(points[i - 1][0]);
					expect(points[i][1]).toBeGreaterThan(points[i - 1][1]);
				}
			}
			expect(
				[...document.querySelectorAll('.stop-name')].some((label) =>
					label.textContent?.includes('(2)')
				)
			).toBe(true);
			await userEvent.click(document.querySelector('#stop_points')!);
			expect(document.querySelectorAll('.layercake-layout-svg circle').length).toBe(7);
		});
	}
}

test('multiple sources keep colliding trip/stop/route IDs separate and use their own route colors', async () => {
	const sources: Source[] = ['mta_subway', 'mta_bus', 'njt_bus'];
	const built = sources.map((source) => chartFixture(source, 0));
	clock.ms = chartNow;
	fixture = {
		catalogs: Object.fromEntries(built.map((b, i) => [sources[i], b.catalog])),
		services: Object.fromEntries(built.map((b, i) => [sources[i], b.service]))
	};
	page.data = { selected_sources: sources };
	await renderChart();
	for (const source of ['mta_bus', 'njt_bus']) {
		// Activate the trigger itself, not a removable chip at its geometric center.
		document.querySelector<HTMLButtonElement>('[aria-label="Add routes"]')!.focus();
		await userEvent.keyboard('{Enter}');
		const option = [...document.querySelectorAll('[role="option"]')].find((option) =>
			option.textContent?.includes(`${source} route`)
		)!;
		await userEvent.click(option);
	}
	await expect.poll(() => document.querySelectorAll('.path-line').length).toBe(9);
	await expectChartSize();
	expect(
		[...document.querySelectorAll('.path-line')].map((path) => path.getAttribute('stroke'))
	).toEqual([
		'#00933c',
		'#00933c',
		'#00933c',
		'#123456',
		'#123456',
		'#123456',
		'#abcdef',
		'#abcdef',
		'#abcdef'
	]);
	expect(document.querySelectorAll('.stop-name').length).toBe(15);
});

test('only intersecting trips are rendered and focusable, including segments crossing either boundary', async () => {
	const times = [
		[-5, -4],
		[4, 5],
		[-4, -2],
		[2, 4],
		[-4, 4],
		[-3, -3],
		[3, 3]
	];
	const built = setFixture(
		'mta_subway',
		0,
		times.map(() => ['a', 'b'])
	);
	const calls = built.service.routes['4'].calls;
	calls.arrival = calls.trip.map(
		(trip, i) => chartNow / 1000 + times[trip][calls.sequence[i]] * 3600
	);
	calls.departure = [...calls.arrival];
	await renderChart();
	await expect.poll(() => document.querySelectorAll('.path-line').length).toBe(5);
	expect(
		[...document.querySelectorAll('.path-hitarea')].map((path) => path.getAttribute('aria-label'))
	).toEqual([2, 3, 4, 5, 6].map((i) => `View details for trip trip-${i}`));
});

test('fully off-domain trips show the empty state without keyboard targets', async () => {
	const built = setFixture('mta_subway', 0, [
		['a', 'b'],
		['a', 'b']
	]);
	const calls = built.service.routes['4'].calls;
	calls.arrival = calls.trip.map((trip) => chartNow / 1000 + (trip ? 5 : -5) * 3600);
	calls.departure = [...calls.arrival];
	await renderChart();
	await expect.poll(() => document.body.textContent).toContain('No data available');
	expect(document.querySelectorAll('.path-hitarea').length).toBe(0);
});

test('ticks label the domain starting day and the next day at midnight', async () => {
	const built = setFixture('mta_subway', 0, [['a', 'b']]);
	clock.ms = new Date(2026, 9, 6, 0, 30).getTime();
	const calls = built.service.routes['4'].calls;
	calls.arrival = calls.sequence.map((sequence) => clock.ms / 1000 + (sequence - 1) * 3600);
	calls.departure = [...calls.arrival];
	await renderChart();
	await expect.poll(() => document.querySelector('.x-axis tspan')?.textContent).toBe('2026-10-06');
	const boundary = document.querySelector('.x-axis tspan')!;
	expect(boundary.parentElement?.textContent).toContain('12:00 AM');
	expect(document.querySelector('.layercake-layout-svg')?.textContent).toContain('2026-10-05');
});

test('SVG export contains wide rendered stop labels without shrinking the chart bounds', async () => {
	const built = setFixture('mta_subway', 0);
	built.stops[0].name = 'W'.repeat(25);
	await renderChart();
	await expectChartSize();
	const svg = document.querySelector<SVGSVGElement>('.layercake-layout-svg')!;
	const labels = svg.querySelector<SVGGElement>('.y-axis')!.getBBox();
	expect(labels.x).toBeLessThan(-160);
	const createURL = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:chart-export');
	const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
	try {
		await userEvent.click(document.querySelector('[aria-label="Export chart as SVG"]')!);
		const blob = createURL.mock.calls[0][0] as Blob;
		const exported = new DOMParser().parseFromString(await blob.text(), 'image/svg+xml');
		const viewBox = exported.documentElement.getAttribute('viewBox')!.split(' ').map(Number);
		const [x, y, width, height] = viewBox;
		expect(x).toBeLessThanOrEqual(labels.x);
		expect(x + width).toBeGreaterThanOrEqual(svg.getBoundingClientRect().width + 10);
		expect(y).toBeLessThanOrEqual(-20);
		expect(y + height).toBeGreaterThanOrEqual(svg.getBoundingClientRect().height + 44);
		expect(exported.querySelector('rect')?.getAttribute('x')).toBe(String(x));
	} finally {
		createURL.mockRestore();
		click.mockRestore();
	}
});

test('a failed refresh keeps the loaded chart and its keyboard targets', async () => {
	setFixture('mta_subway', 0);
	const fetcher = fixtureFetcher(fixture);
	let fail = false;
	const store = new TransitData({
		sources: ['mta_subway'],
		fetcher: (...args) => (fail ? Promise.reject(new TypeError('Offline')) : fetcher(...args))
	});
	await renderChart(store);
	await expect.poll(() => document.querySelectorAll('.path-line').length).toBe(3);
	const paths = [...document.querySelectorAll('.path-line')].map((path) => path.getAttribute('d'));
	fail = true;
	store.retry();
	await expect.poll(() => store.status.warning).toBe(true);
	expect(
		[...document.querySelectorAll('.path-line')].map((path) => path.getAttribute('d'))
	).toEqual(paths);
	expect(document.querySelectorAll('.path-hitarea').length).toBe(3);
});
