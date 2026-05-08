import { Order } from "effect";

export const optionalOrder = <A>(ao: Order.Order<A>): Order.Order<A | undefined> =>
	Order.make((x, y) => {
		const xn = x === undefined;
		const yn = y === undefined;

		return xn && yn ? 0 : xn ? -1 : yn ? 1 : ao(x, y);
	});
