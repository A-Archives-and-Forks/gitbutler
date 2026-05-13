import { type Operand } from "#ui/operands.ts";
import { parseDragData } from "./OperationSourceC.tsx";
import styles from "./OperationTarget.module.css";
import { OperationTooltip } from "./OperationTooltip.tsx";
import {
	getOperation,
	getOperations,
	type OperationType,
	useRunOperationMutationOptions,
} from "#ui/operations/operation.ts";
import { classes } from "#ui/ui/classes.ts";
import { projectActions, selectProjectOutlineModeState } from "#ui/projects/state.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { dropTargetForElements } from "@atlaskit/pragmatic-drag-and-drop/element/adapter";
import {
	attachInstruction,
	extractInstruction,
} from "@atlaskit/pragmatic-drag-and-drop-hitbox/list-item";
import { mergeProps, useRender } from "@base-ui/react";
import { Match, pipe } from "effect";
import { FC, useEffect, useEffectEvent, useRef } from "react";
import { useMutation } from "@tanstack/react-query";

type DropTargetParams = Parameters<typeof dropTargetForElements>[0];
type GetDataArgs = Parameters<NonNullable<DropTargetParams["getData"]>>[0];

type DropData = {
	operationType: OperationType;
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
	const { mutate: runOperation } = useMutation(useRunOperationMutationOptions());
	const dropRef = useRef<HTMLElement>(null);

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

		return { operationType };
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

				if (!isActiveDropTarget) return;

				const dropData = parseDropData(args.self.data);

				dispatch(
					projectActions.updatePointerTransfer({
						projectId,
						target: dropData ? target : null,
						operationType: dropData?.operationType ?? null,
					}),
				);
			},
			onDragLeave: () => {
				dispatch(
					projectActions.updatePointerTransfer({
						projectId,
						target: null,
						operationType: null,
					}),
				);
			},
			onDrop: (args) => {
				const [innerMost] = args.location.current.dropTargets;
				const isActiveDropTarget = innerMost?.element === args.self.element;

				if (!isActiveDropTarget) return;

				const dragData = parseDragData(args.source.data);
				const dropData = parseDropData(args.self.data);
				const operation =
					dragData && dropData
						? getOperation({
								source: dragData.source,
								target,
								operationType: dropData.operationType,
							})
						: null;

				if (!operation) {
					dispatch(projectActions.cancelMode({ projectId }));
					return;
				}

				dispatch(projectActions.exitMode({ projectId }));
				runOperation(operation);
			},
		});
	}, [dispatch, projectId, runOperation, target]);

	return { dropRef };
};

export const OperationTarget: FC<
	{
		target: Operand;
		projectId: string;
		isSelected: boolean;
		isAbsorptionTarget: boolean;
	} & useRender.ComponentProps<"div">
> = ({ target, projectId, isSelected, isAbsorptionTarget, render, ...props }) => {
	const { dropRef } = useOperationDropTarget({ target, projectId });
	const outlineMode = useAppSelector((state) => selectProjectOutlineModeState(state, projectId));

	const insertTargetOperationType = Match.value(outlineMode).pipe(
		Match.tag("Transfer", ({ value: mode }) =>
			isSelected && (mode.operationType === "moveAbove" || mode.operationType === "moveBelow")
				? mode.operationType
				: null,
		),
		Match.orElse(() => null),
	);

	const isMainTargetActive = Match.value(outlineMode).pipe(
		Match.tags({
			Absorb: () => isAbsorptionTarget,
			Transfer: ({ value: mode }) => isSelected && mode.operationType === "rub",
		}),
		Match.orElse(() => false),
	);

	const isMainTargetTooltipActive = Match.value(outlineMode).pipe(
		Match.tags({
			Absorb: () => isSelected,
			Transfer: () => isMainTargetActive,
		}),
		Match.orElse(() => false),
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
				isActive={isMainTargetTooltipActive}
				outlineMode={outlineMode}
				render={targetEl}
			/>

			{insertTargetOperationType !== null && (
				<OperationTooltip
					projectId={projectId}
					target={target}
					isActive
					outlineMode={outlineMode}
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
