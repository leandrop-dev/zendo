// The grid's arithmetic, apart from the components so it can be tested: the
// cards' fixed size (as they are drawn), how many fit across, and
// which rows a scroll position shows.

/// A card's width and height, and the gap between cards.
export const CARD_WIDTH = 192;
export const CARD_HEIGHT = 370;
export const GAP = 16;
/// The room around the grid, and the scroll bar's.
export const MARGIN = 24;
export const BAR = 14;
/// The rows the grid draws beyond the one on screen, each way.
export const OVERSCAN = 2;

/// How many cards fit across a window `width` wide.
export function columnsFor(width: number): number {
	if (width <= 0) {
		return 1;
	}
	return Math.max(1, Math.floor((width - 2 * MARGIN - BAR + GAP) / (CARD_WIDTH + GAP)));
}

/// The rows the grid has for `count` cards across `columns`.
export function rowsFor(count: number, columns: number): number {
	return Math.ceil(count / Math.max(1, columns));
}

/// The rows to draw for a scroll position and a viewport, with the rows just
/// outside it so a scroll does not show a blank strip.
export function visibleRows(
	scrollTop: number,
	viewportHeight: number,
	rows: number
): { first: number; last: number } {
	const pitch = CARD_HEIGHT + GAP;
	const first = Math.max(0, Math.floor(scrollTop / pitch) - OVERSCAN);
	const last = Math.min(rows, Math.ceil((scrollTop + viewportHeight) / pitch) + OVERSCAN);
	return { first, last };
}

/// Where a row's cards start, so the grid is centred and a short last row
/// keeps its columns.
export function leftFor(width: number, columns: number): number {
	return Math.max(0, (width - BAR - (columns * (CARD_WIDTH + GAP) - GAP)) / 2);
}
