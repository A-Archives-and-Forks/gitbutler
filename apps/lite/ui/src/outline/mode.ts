import { Match } from "effect";
import {
	BranchOperand,
	branchOperand,
	CommitOperand,
	commitOperand,
	operandContains,
	operandEquals,
	type Operand,
} from "#ui/operands.ts";
import { getOperation, getOperations, OperationType } from "#ui/operations/operation.ts";
import { filterNavigationIndex, NavigationIndex } from "#ui/workspace/navigation-index.ts";
import { CommitAbsorption } from "@gitbutler/but-sdk";

/** @public */
export type AbsorbMode = {
	source: Operand;
	absorptionPlan: Array<CommitAbsorption>;
};

/** @public */
export type TransferMode = {
	value: TransferOperationMode;
};

/** @public */
export type KeyboardTransferOperationMode = {
	source: Operand;
	operationType: OperationType;
};

/** @public */
export type PointerTransferOperationMode = {
	source: Operand;
	operationType: OperationType | null;
};

/** @public */
export type TransferOperationMode =
	| ({ _tag: "Keyboard" } & KeyboardTransferOperationMode)
	| ({ _tag: "Pointer" } & PointerTransferOperationMode);

/** @public */
export const keyboardTransferOperationMode = ({
	source,
	operationType,
}: KeyboardTransferOperationMode): TransferOperationMode => ({
	_tag: "Keyboard",
	source,
	operationType,
});

/** @public */
export const pointerTransferOperationMode = ({
	source,
	operationType,
}: PointerTransferOperationMode): TransferOperationMode => ({
	_tag: "Pointer",
	source,
	operationType,
});

/** @public */
export const absorbOutlineMode = ({ source, absorptionPlan }: AbsorbMode): OutlineMode => ({
	_tag: "Absorb",
	source,
	absorptionPlan,
});

/** @public */
export const transferOutlineMode = ({ value }: TransferMode): OutlineMode => ({
	_tag: "Transfer",
	value,
});

/** @public */
export type RewordCommitOutlineMode = { operand: CommitOperand };
/** @public */
export type RenameBranchOutlineMode = { operand: BranchOperand };
export type OutlineMode =
	| { _tag: "Default" }
	| ({ _tag: "RewordCommit" } & RewordCommitOutlineMode)
	| ({ _tag: "RenameBranch" } & RenameBranchOutlineMode)
	| ({ _tag: "Absorb" } & AbsorbMode)
	| ({ _tag: "Transfer" } & TransferMode);

/** @public */
export const defaultOutlineMode: OutlineMode = {
	_tag: "Default",
};

/** @public */
export const rewordCommitOutlineMode = ({ operand }: RewordCommitOutlineMode): OutlineMode => ({
	_tag: "RewordCommit",
	operand,
});

/** @public */
export const renameBranchOutlineMode = ({ operand }: RenameBranchOutlineMode): OutlineMode => ({
	_tag: "RenameBranch",
	operand,
});

export const isValidOutlineModeForSelection = ({
	mode,
	selection,
}: {
	mode: OutlineMode;
	selection: Operand;
}): boolean =>
	Match.value(mode).pipe(
		Match.tagsExhaustive({
			Default: () => true,
			Absorb: () => true,
			Transfer: () => true,
			RewordCommit: (mode) => operandEquals(selection, commitOperand(mode.operand)),
			RenameBranch: (mode) => operandEquals(selection, branchOperand(mode.operand)),
		}),
	);

export const getTransferOperation = ({
	mode,
	target,
}: {
	mode: TransferOperationMode;
	target: Operand;
}) => {
	const { operationType } = mode;
	if (operationType === null) return null;
	return getOperation({
		source: mode.source,
		target,
		operationType,
	});
};

const hasAnyOperation = (source: Operand, target: Operand) => {
	const operations = getOperations(source, target);
	return !!operations.rub || !!operations.moveAbove || !!operations.moveBelow;
};

export const isOutlineModeCandidateTarget = ({
	mode,
	target,
}: {
	mode: OutlineMode;
	target: Operand;
}): boolean =>
	Match.value(mode).pipe(
		Match.tags({
			Absorb: ({ absorptionPlan }) =>
				absorptionPlan.some(({ stackId, commitId }) =>
					operandEquals(commitOperand({ stackId, commitId }), target),
				),
			Transfer: ({ value: mode }) => hasAnyOperation(mode.source, target),
		}),
		Match.orElse(() => false),
	);

export const filterNavigationIndexForOutlineMode = ({
	navigationIndex: navigationIndexUnfiltered,
	outlineMode,
}: {
	navigationIndex: NavigationIndex;
	outlineMode: OutlineMode;
}) =>
	Match.value(outlineMode).pipe(
		Match.tagsExhaustive({
			Default: () => navigationIndexUnfiltered,
			Absorb: (activeMode) =>
				filterNavigationIndex(
					navigationIndexUnfiltered,
					(operand) =>
						operandContains(operand, activeMode.source) ||
						isOutlineModeCandidateTarget({ mode: activeMode, target: operand }),
				),
			Transfer: (activeMode) =>
				filterNavigationIndex(
					navigationIndexUnfiltered,
					(operand) =>
						operandContains(operand, activeMode.value.source) ||
						isOutlineModeCandidateTarget({ mode: activeMode, target: operand }),
				),
			RenameBranch: (x) =>
				filterNavigationIndex(navigationIndexUnfiltered, (operand) =>
					operandEquals(operand, branchOperand(x.operand)),
				),
			RewordCommit: (x) =>
				filterNavigationIndex(navigationIndexUnfiltered, (operand) =>
					operandEquals(operand, commitOperand(x.operand)),
				),
		}),
	);
