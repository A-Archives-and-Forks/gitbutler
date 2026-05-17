// https://linear.app/gitbutler/issue/GB-1161/refsbranches-should-use-bytes-instead-of-strings
export const decodeRefName = (fullNameBytes: Array<number>): string =>
	new TextDecoder().decode(Uint8Array.from(fullNameBytes));

export const encodeRefName = (fullName: string): Array<number> =>
	Array.from(new TextEncoder().encode(fullName));

export const refNamesEqual = (left: Array<number> | null, right: Array<number> | null): boolean =>
	left === right ||
	(left !== null &&
		right !== null &&
		left.length === right.length &&
		left.every((value, index) => value === right[index]));
