import { createRoute, notFound, Outlet } from "@tanstack/react-router";

import { Route as rootRoute } from "#ui/routes/__root.tsx";
import { handleWatcher } from "#ui/watcher.ts";

export const Route = createRoute({
	getParentRoute: () => rootRoute,
	path: "project/$id",
	remountDeps: ({ params }) => params.id,
	// Needed for `remountDeps` to work.
	component: () => <Outlet />,
	beforeLoad: ({ matches, routeId }) => {
		// We don't want an index route.
		if (matches.at(-1)?.routeId === routeId) throw notFound();
	},
	loader: async ({ params, context }) => {
		const subscriptionId = await window.lite.watcherSubscribe(params.id, (event) =>
			handleWatcher(event, params.id, context.queryClient),
		);
		return { subscriptionId };
	},
	onLeave: ({ loaderData }) => {
		if (loaderData) void window.lite.watcherUnsubscribe(loaderData.subscriptionId);
	},
});
