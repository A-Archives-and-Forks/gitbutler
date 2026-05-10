import type { Code } from "@gitbutler/but-sdk";

export type ReduxError = {
	name?: string;
	message: string;
	code?: Code;
};

export function isReduxError(something: unknown): something is ReduxError {
	if (!something || typeof something !== "object") return false;
	const r = something as ReduxError;
	return (
		typeof r.message === "string" &&
		(r.name === undefined || typeof r.name === "string") &&
		(r.code === undefined || typeof r.code === "string")
	);
}
