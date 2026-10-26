import '../../app.css';

import { chartFixture } from '#lib/charts/fixtures.js';

import { expect, test } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { page } from 'vitest/browser';

import PlatformExits from './PlatformExits.svelte';

function train(car_count: number) {
	const trip = chartFixture('mta_subway').trips[0];
	if (trip.data.source !== 'mta_subway') throw new Error('Expected subway');
	trip.data.consist = { car_count, car_length_feet: 60 };
	return trip;
}

test('keeps car positions visible alongside distinct fleet IDs and centers positions without IDs', async () => {
	const trip = train(3);
	if (trip.data.source !== 'mta_subway') throw new Error('Expected subway');
	trip.data.consist_cars = [
		{ number: '7244', type: 'R142' },
		{ number: '2', type: 'R142' }
	];
	const view = await render(PlatformExits, { trip, guidance: null });
	try {
		const front = view.container.querySelector('[data-train-car="1"]')!;
		expect(front.querySelectorAll('text')).toHaveLength(2);
		expect(front.querySelector('.car-pos')?.textContent).toBe('1');
		expect(front.querySelector('.car-id')?.textContent).toBe('7244');
		const position = front.querySelector('.car-pos')!.getBoundingClientRect();
		const fleet_id = front.querySelector('.car-id')!.getBoundingClientRect();
		expect(position.bottom).toBeLessThan(fleet_id.top);
		for (const number of [2, 3]) {
			const car = view.container.querySelector(`[data-train-car="${number}"]`)!;
			expect(car.querySelectorAll('text')).toHaveLength(1);
			expect(car.querySelector('.car-pos')?.textContent).toBe(String(number));
			expect(car.querySelector('text')?.getAttribute('y')).toBe('16');
		}
	} finally {
		await view.unmount();
	}
});

for (const width of [320, 390, 800])
	for (const long_names of [false, true]) {
		test(`${long_names ? 'long' : 'short'} named exit labels fit and stay separate at ${width}px`, async () => {
			await page.viewport(width, 844);
			const view = await render(PlatformExits, {
				trip: train(10),
				guidance: {
					car_count: 10,
					exits: [
						{
							id: 'rear',
							name: long_names
								? 'Westchester Avenue & SB Bruckner Blvd/Amendola Plaza'
								: 'Utica Av',
							accessible: false,
							car: 10,
							position: 0.0283
						},
						{
							id: 'elevator',
							name: long_names
								? 'Westchester Avenue & SB Bruckner Blvd/Amendola Plaza'
								: 'Utica Av',
							accessible: true,
							car: 8,
							position: 0.2567
						},
						{
							id: 'front',
							name: long_names
								? 'Penn Station Hilton Passageway (LIRR, NJ Transit, Amtrak)'
								: 'Stuyvesant Av',
							accessible: false,
							car: 1,
							position: 0.9783
						}
					]
				}
			});
			await expect
				.poll(() => {
					const section = view.container.querySelector('.diagram')!.getBoundingClientRect();
					const boxes = view
						.getByRole('listitem')
						.all()
						.map((button) => button.element().getBoundingClientRect());
					if (
						boxes.some(
							(box) =>
								box.left < section.left || box.right > section.right || box.bottom > section.bottom
						)
					)
						return false;
					for (let i = 0; i < boxes.length; i++)
						for (let j = i + 1; j < boxes.length; j++) {
							const a = boxes[i],
								b = boxes[j];
							if (!(
								a.right <= b.left ||
								b.right <= a.left ||
								a.bottom <= b.top ||
								b.bottom <= a.top
							))
								return false;
						}
					return true;
				})
				.toBe(true);
			await view.unmount();
		});
	}

for (const car_count of [4, 10]) {
	test(`${car_count}-car trains keep readable car widths and scroll on narrow screens`, async () => {
		await page.viewport(1000, 844);
		const view = await render(PlatformExits, {
			trip: train(car_count),
			guidance: {
				car_count,
				exits: [
					{ id: 'end', name: 'Barclays Center', accessible: false, car: car_count, position: 0 }
				]
			}
		});
		const scroller = view
			.getByRole('region', { name: 'Train and platform exits; scroll for more' })
			.element();
		await expect
			.poll(() =>
				[...view.container.querySelectorAll('[data-train-car]')].every(
					(car) => car.getBoundingClientRect().width >= 50
				)
			)
			.toBe(true);
		expect(scroller.scrollWidth).toBe(scroller.clientWidth);
		await page.viewport(320, 844);
		await expect
			.poll(() =>
				[...view.container.querySelectorAll('[data-train-car]')].every(
					(car) => car.getBoundingClientRect().width >= 50
				)
			)
			.toBe(true);
		await expect.poll(() => scroller.scrollWidth > scroller.clientWidth).toBe(car_count === 10);
		if (car_count === 10) {
			scroller.scrollLeft = 100;
			expect(scroller.scrollLeft).toBe(100);
		}
		expect(view.container.querySelector('[data-exit-shaft]')?.getAttribute('d')).toMatch(/^M6 25/);
		await view.unmount();
	});
}
