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
export type RubOperationMode = { source: Operand };
/** @public */
export type CutOperationMode = { source: Operand };
/** @public */
export type MoveOperationMode = { source: Operand };
/** @public */
export type DragAndDropOperationMode = { source: Operand; operationType: OperationType | null };
export type OperationMode =
	| ({ _tag: "Absorb" } & AbsorbOperationMode)
	| ({ _tag: "Rub" } & RubOperationMode)
	| ({ _tag: "Cut" } & CutOperationMode)
	| ({ _tag: "Move" } & MoveOperationMode)
	| ({ _tag: "DragAndDrop" } & DragAndDropOperationMode);

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
export const rubOperationMode = ({ source }: RubOperationMode): OperationMode => ({
	_tag: "Rub",
	source,
});

/** @public */
export const cutOperationMode = ({ source }: CutOperationMode): OperationMode => ({
	_tag: "Cut",
	source,
});

/** @public */
export const moveOperationMode = ({ source }: MoveOperationMode): OperationMode => ({
	_tag: "Move",
	source,
});

/** @public */
export const dragAndDropOperationMode = ({
	source,
	operationType,
}: DragAndDropOperationMode): OperationMode => ({
	_tag: "DragAndDrop",
	source,
	operationType,
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

export const operationModeToOperationType = (operationMode: OperationMode): OperationType | null =>
	Match.value(operationMode).pipe(
		Match.withReturnType<OperationType | null>(),
		Match.tags({
			Absorb: () => null,
			Rub: () => "rub",
			Cut: () => null,
			// We should have the ability to move either above or below.
			Move: ({ source }) => (source._tag === "Branch" ? "moveAbove" : "moveBelow"),
			DragAndDrop: ({ operationType }) => operationType,
		}),
		Match.exhaustive,
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

const hasAnyOperation = (source: Operand, target: Operand) => {
	const operations = getOperations(source, target);
	return !!operations.rub || !!operations.moveAbove || !!operations.moveBelow;
};

const operationModeHasOperation = ({
	mode,
	operand,
}: {
	mode: OperationMode;
	operand: Operand;
}): boolean =>
	Match.value(mode).pipe(
		Match.tagsExhaustive({
			Absorb: ({ absorptionPlan }) =>
				absorptionPlan.some(({ stackId, commitId }) =>
					operandEquals(commitOperand({ stackId, commitId }), operand),
				),
			DragAndDrop: ({ source }) => hasAnyOperation(source, operand),
			Cut: ({ source }) => hasAnyOperation(source, operand),
			Move: (mode) =>
				!!getOperation({
					source: mode.source,
					target: operand,
					operationType: operationModeToOperationType(mode),
				}),
			Rub: (mode) =>
				!!getOperation({
					source: mode.source,
					target: operand,
					operationType: operationModeToOperationType(mode),
				}),
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
						operandContains(operand, operationMode.source) ||
						(operationMode._tag === "Absorb" &&
							operationMode.absorptionPlan.some(({ stackId, commitId }) =>
								operandEquals(commitOperand({ stackId, commitId }), operand),
							)) ||
						operationModeHasOperation({ mode: operationMode, operand }),
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
