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
export type AbsorbOperationMode = {
	source: Operand;
	absorptionPlan: Array<CommitAbsorption>;
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

export type OperationMode =
	| ({ _tag: "Absorb" } & AbsorbOperationMode)
	| { _tag: "Transfer"; value: TransferOperationMode };

/** @public */
export const absorbOperationMode = ({
	source,
	absorptionPlan,
}: AbsorbOperationMode): OperationMode => ({
	_tag: "Absorb",
	source,
	absorptionPlan,
});

/** @public */
export const transferOperationMode = (value: TransferOperationMode): OperationMode => ({
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
	| { _tag: "Operation"; value: OperationMode };

/** @public */
export const defaultOutlineMode: OutlineMode = {
	_tag: "Default",
};

/** @public */
export const operationOutlineMode = (value: OperationMode): OutlineMode => ({
	_tag: "Operation",
	value,
});

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

export const getOperationMode = (mode: OutlineMode): OperationMode | null =>
	Match.value(mode).pipe(
		Match.withReturnType<OperationMode | null>(),
		Match.tags({ Operation: ({ value }) => value }),
		Match.orElse(() => null),
	);

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
			Operation: () => true,
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

const operationModeSource = (mode: OperationMode): Operand =>
	Match.value(mode).pipe(
		Match.tagsExhaustive({
			Absorb: ({ source }) => source,
			Transfer: ({ value: mode }) => mode.source,
		}),
	);

export const isOperationModeCandidateTarget = ({
	mode,
	target,
}: {
	mode: OperationMode;
	target: Operand;
}): boolean =>
	Match.value(mode).pipe(
		Match.tagsExhaustive({
			Absorb: ({ absorptionPlan }) =>
				absorptionPlan.some(({ stackId, commitId }) =>
					operandEquals(commitOperand({ stackId, commitId }), target),
				),
			Transfer: ({ value: mode }) => hasAnyOperation(mode.source, target),
		}),
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
			Operation: ({ value: operationMode }) =>
				filterNavigationIndex(
					navigationIndexUnfiltered,
					(operand) =>
						operandContains(operand, operationModeSource(operationMode)) ||
						isOperationModeCandidateTarget({ mode: operationMode, target: operand }),
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
