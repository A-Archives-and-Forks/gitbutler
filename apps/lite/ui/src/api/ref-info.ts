import { encodeRefName, refNamesEqual } from "#ui/api/ref-name.ts";
import { type Commit, type RefInfo, type RelativeTo, type Segment } from "@gitbutler/but-sdk";

export const getCommonBaseCommitId = (headInfo: RefInfo): string | undefined => {
	const bases = headInfo.stacks
		.map((stack) => stack.base)
		.filter((base): base is string => base !== null);
	const first = bases[0];
	if (first === undefined) return undefined;
	return bases.every((base) => base === first) ? first : undefined;
};

export const getBranchNameByCommitId = (headInfo: RefInfo): Map<string, string | undefined> => {
	const byCommitId = new Map<string, string | undefined>();

	for (const stack of headInfo.stacks)
		for (const segment of stack.segments) {
			const branchName = segment.refName?.displayName;
			for (const commit of segment.commits) byCommitId.set(commit.id, branchName);
		}

	return byCommitId;
};

export const findCommit = ({
	headInfo,
	commitId,
}: {
	headInfo: RefInfo;
	commitId: string;
}): Commit | null => {
	for (const stack of headInfo.stacks)
		for (const segment of stack.segments) {
			const commit = segment.commits.find((candidate) => candidate.id === commitId);
			if (!commit) continue;

			return commit;
		}

	return null;
};

export const findSegmentByBranchRef = ({
	headInfo,
	branchRef,
}: {
	headInfo: RefInfo;
	branchRef: Array<number> | null;
}): Segment | null => {
	for (const stack of headInfo.stacks)
		for (const segment of stack.segments)
			if (refNamesEqual(segment.refName?.fullNameBytes ?? null, branchRef)) return segment;

	return null;
};

export const resolveRelativeTo = ({
	headInfo,
	relativeTo,
}: {
	headInfo: RefInfo;
	relativeTo: RelativeTo;
}): string | null => {
	switch (relativeTo.type) {
		case "commit":
			return relativeTo.subject;
		case "referenceBytes":
			return (
				findSegmentByBranchRef({ headInfo, branchRef: relativeTo.subject })?.commits[0]?.id ?? null
			);
		case "reference":
			return (
				findSegmentByBranchRef({ headInfo, branchRef: encodeRefName(relativeTo.subject) })
					?.commits[0]?.id ?? null
			);
	}
};
