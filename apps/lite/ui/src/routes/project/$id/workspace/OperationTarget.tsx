import { commitOperand, operandEquals, type Operand } from "#ui/operands.ts";
import { parseDragData } from "./OperationSourceC.tsx";
import styles from "./OperationTarget.module.css";
import { OperationTooltip } from "./OperationTooltip.tsx";
import {
	getOperation,
	getOperations,
	OperationType,
	useRunOperation,
} from "#ui/operations/operation.ts";
import { classes } from "#ui/ui/classes.ts";
import { projectActions, selectProjectOperationModeState } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { dropTargetForElements } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import {
	attachInstruction,
	extractInstruction,
} from "@atlaskit/pragmatic-drag-and-drop-hitbox/list-item";
import { mergeProps, useRender } from "@base-ui/react";
import { Match, pipe } from "effect";
import { FC, useEffect, useEffectEvent, useRef, useState } from "react";

type DropTargetParams = Parameters<typeof dropTargetForElements>[0];
type GetDataArgs = Parameters<NonNullable<DropTargetParams["getData"]>>[0];

type DropData = {
	operationType: OperationType;
	target: Operand;
};

const parseDropData = (data: unknown): DropData | null => {
	if (typeof data !== "object" || data === null || !("operationType" in data)) return null;
	return data as DropData;
};

const getDropOperationType = ({
	source,
	target,
	input,
	element,
}: {
	source: Operand;
	target: Operand;
	input: Parameters<typeof attachInstruction>[1]["input"];
	element: Parameters<typeof attachInstruction>[1]["element"];
}): OperationType | null => {
	const { rub, moveAbove, moveBelow } = getOperations(source, target);

	const instruction = extractInstruction(
		attachInstruction(
			{},
			{
				input,
				element,
				operations: {
					"reorder-before": moveAbove ? "available" : "not-available",
					"reorder-after": moveBelow ? "available" : "not-available",
					combine: rub ? "available" : "not-available",
				},
			},
		),
	);
	if (!instruction) return null;

	return Match.value(instruction.operation).pipe(
		Match.withReturnType<OperationType | null>(),
		Match.when("combine", () => "rub"),
		Match.when("reorder-before", () => "moveAbove"),
		Match.when("reorder-after", () => "moveBelow"),
		Match.exhaustive,
	);
};

const useOperationDropTarget = ({ target, projectId }: { target: Operand; projectId: string }) => {
	const dispatch = useAppDispatch();
	const runOperation = useRunOperation();
	const dropRef = useRef<HTMLElement>(null);
	const [isActiveDropTarget, setIsActiveDropTarget] = useState<boolean>(false);

	const getDropData = useEffectEvent(({ input, element, source }: GetDataArgs): DropData | null => {
		const dragData = parseDragData(source.data);
		if (!dragData) return null;

		const operationType = getDropOperationType({
			source: dragData.source,
			target,
			input,
			element,
		});
		if (operationType === null) return null;

		return { operationType, target };
	});

	useEffect(() => {
		const element = dropRef.current;
		if (!element) return;

		return dropTargetForElements({
			element,
			getData: (args) => getDropData(args) ?? {},
			canDrop: (args) => getDropData(args) !== null,
			onDrag: (args) => {
				const [innerMost] = args.location.current.dropTargets;
				const isActiveDropTarget = innerMost?.element === args.self.element;

				setIsActiveDropTarget(isActiveDropTarget);

				if (!isActiveDropTarget) return;

				const dropData = parseDropData(args.self.data);

				dispatch(
					projectActions.updateDragAndDropMode({
						projectId,
						operationType: dropData?.operationType ?? null,
					}),
				);
			},
			onDragLeave: () => {
				setIsActiveDropTarget(false);
			},
			onDrop: (args) => {
				setIsActiveDropTarget(false);

				const [innerMost] = args.location.current.dropTargets;
				const isActiveDropTarget = innerMost?.element === args.self.element;

				if (!isActiveDropTarget) return;

				const dragData = parseDragData(args.source.data);
				if (!dragData) return;

				const dropData = parseDropData(args.self.data);
				if (!dropData) return;

				const operation = getOperation({
					source: dragData.source,
					target: dropData.target,
					operationType: dropData.operationType,
				});
				if (!operation) return;

				runOperation(projectId, operation);
			},
		});
	}, [dispatch, projectId, runOperation]);

	return { dropRef, isActiveDropTarget };
};

export const OperationTarget: FC<
	{
		target: Operand;
		projectId: string;
		isSelected: boolean;
	} & useRender.ComponentProps<"div">
> = ({ target, projectId, isSelected, render, ...props }) => {
	const { dropRef, isActiveDropTarget } = useOperationDropTarget({ target, projectId });
	const operationMode = useAppSelector((state) =>
		selectProjectOperationModeState(state, projectId),
	);

	const insertTargetOperationType = operationMode
		? Match.value(operationMode).pipe(
				Match.tagsExhaustive({
					DragAndDrop: ({ operationType }) =>
						isActiveDropTarget && (operationType === "moveAbove" || operationType === "moveBelow")
							? operationType
							: null,
					Absorb: () => null,
					Cut: () => null,
					Rub: () => null,
					Move: () => null,
				}),
			)
		: null;

	const isMainTargetActive =
		!!operationMode &&
		Match.value(operationMode).pipe(
			Match.tagsExhaustive({
				DragAndDrop: ({ operationType }) => isActiveDropTarget && operationType === "rub",
				Absorb: ({ absorptionPlan }) =>
					absorptionPlan.some(({ stackId, commitId }) =>
						operandEquals(commitOperand({ stackId, commitId }), target),
					),
				Cut: () => isSelected,
				Rub: () => isSelected,
				Move: () => isSelected,
			}),
		);

	const targetEl = useRender({
		render,
		ref: dropRef,
		props: mergeProps<"div">(props, {
			className: classes(isMainTargetActive && styles.activeTarget),
		}),
	});

	return (
		<div className={styles.target}>
			<OperationTooltip
				projectId={projectId}
				target={target}
				isActive={isSelected}
				operationMode={operationMode}
				render={targetEl}
			/>

			{insertTargetOperationType !== null && (
				<OperationTooltip
					projectId={projectId}
					target={target}
					isActive
					operationMode={operationMode}
					className={classes(
						styles.insertionTarget,
						pipe(
							insertTargetOperationType,
							Match.value,
							Match.when("moveAbove", () => styles.insertionTargetAbove),
							Match.when("moveBelow", () => styles.insertionTargetBelow),
							Match.exhaustive,
						),
					)}
				/>
			)}
		</div>
	);
};
