import { mutationOptions } from "@tanstack/react-query";

/** @public */
export const applyBranchMutationOptions = mutationOptions({
	mutationFn: window.lite.apply,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});

export const commitInsertBlankMutationOptions = mutationOptions({
	mutationFn: window.lite.commitInsertBlank,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});

export const commitCreateMutationOptions = mutationOptions({
	mutationFn: window.lite.commitCreate,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});

export const commitDiscardMutationOptions = mutationOptions({
	mutationFn: window.lite.commitDiscard,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});

export const commitRewordMutationOptions = mutationOptions({
	mutationFn: window.lite.commitReword,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});

export const updateBranchNameMutationOptions = mutationOptions({
	mutationFn: window.lite.updateBranchName,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});

export const unapplyStackMutationOptions = mutationOptions({
	mutationFn: window.lite.unapplyStack,
	onSuccess: async (_data, _input, _ctx, { client }) => {
		await client.invalidateQueries();
	},
});
