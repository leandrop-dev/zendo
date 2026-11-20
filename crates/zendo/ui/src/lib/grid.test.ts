import { describe, expect, it } from 'vitest';
import { columnsFor, leftFor, rowsFor, visibleRows } from './grid.js';

describe('the grid', () => {
	it('fits the cards across a window, 192 wide with 16 between', () => {
		// 1360 wide: 1360 - 48 - 14 + 16 = 1314, over 208 per card and gap.
		expect(columnsFor(1360)).toBe(6);
		expect(columnsFor(2560)).toBe(12);
		expect(columnsFor(0)).toBe(1);
		expect(columnsFor(120)).toBe(1);
	});

	it('counts the rows a list needs', () => {
		expect(rowsFor(2700, 6)).toBe(450);
		expect(rowsFor(0, 6)).toBe(0);
	});

	it('draws the rows on screen and two each side', () => {
		// 860 high: rows 0, 1 and 2 are on screen (386 each), and two more.
		expect(visibleRows(0, 860, 450)).toEqual({ first: 0, last: 5 });
		expect(visibleRows(386 * 10, 860, 450)).toEqual({ first: 8, last: 15 });
		expect(visibleRows(386 * 449, 860, 450)).toEqual({ first: 447, last: 450 });
	});

	it('centres the row in the width', () => {
		// 6 cards and 5 gaps: 6 * 208 - 16 = 1232; 1360 - 14 - 1232 = 114, over 2.
		expect(leftFor(1360, 6)).toBe(57);
		// 300 wide, one card: (300 - 14 - 192) / 2.
		expect(leftFor(300, 1)).toBe(47);
	});
});
