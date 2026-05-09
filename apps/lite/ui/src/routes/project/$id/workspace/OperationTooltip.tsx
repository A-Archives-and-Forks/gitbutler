import { classes } from "#ui/ui/classes.ts";
import {
	getOperation,
	getOperations,
	operationLabel,
	useRunOperation,
	type Operation,
	type OperationsByType,
} from "#ui/operations/operation.ts";
import { ShortcutButton } from "#ui/ui/ShortcutButton.tsx";
import uiStyles from "#ui/ui/ui.module.css";
import { Tooltip, useRender } from "@base-ui/react";
import { FC } from "react";
import styles from "./OperationTooltip.module.css";
import { Operand, operandEquals } from "#ui/operands.ts";
import { useAppDispatch } from "#ui/store.ts";
import { projectActions } from "#ui/projects/state.ts";
import { operationModeToOperationType, OperationMode } from "#ui/outline/mode.ts";
import { Match } from "effect";
import { useCommand } from "#ui/commands/manager.ts";

const OperationModeControls: FC<{
	projectId: string;
	operation: Operation | null;
}> = ({ projectId, operation }) => {
	const dispatch = useAppDispatch();
	const runOperation = useRunOperation();

	const confirm = () => {
		dispatch(projectActions.exitMode({ projectId }));

		if (!operation) return;

		runOperation(projectId, operation);
	};

	const cancel = () => dispatch(projectActions.exitMode({ projectId }));

	const confirmCommand = useCommand(confirm, {
		enabled: !!operation,
		commandPalette: { group: "Operation mode", label: "Confirm" },
		shortcutsBar: { label: "Confirm" },
		hotkeys: [{ hotkey: "Enter" }],
	});

	const cancelCommand = useCommand(cancel, {
		commandPalette: { group: "Operation mode", label: "Cancel" },
		shortcutsBar: { label: "Cancel" },
		hotkeys: [{ hotkey: "Escape" }],
	});

	return (
		<>
			{operation && (
				<ShortcutButton
					className={uiStyles.button}
					hotkeys={confirmCommand.hotkeys}
					onClick={confirmCommand.commandFn}
				>
					{operationLabel(operation)}
				</ShortcutButton>
			)}
			<ShortcutButton
				className={uiStyles.button}
				hotkeys={cancelCommand.hotkeys}
				onClick={cancelCommand.commandFn}
			>
				Cancel
			</ShortcutButton>
		</>
	);
};

const CutOperationControls: FC<{
	projectId: string;
	operations: OperationsByType;
}> = ({ projectId, operations }) => {
	const dispatch = useAppDispatch();
	const runOperation = useRunOperation();

	const run = (operation: Operation | null) => {
		dispatch(projectActions.exitMode({ projectId }));

		if (!operation) return;

		runOperation(projectId, operation);
	};

	const cancel = () => dispatch(projectActions.exitMode({ projectId }));

	const moveAboveCommand = useCommand(() => run(operations.moveAbove), {
		enabled: !!operations.moveAbove,
		commandPalette: operations.moveAbove
			? { group: "Operation mode", label: operationLabel(operations.moveAbove) }
			: undefined,
		shortcutsBar: operations.moveAbove
			? { label: operationLabel(operations.moveAbove) }
			: undefined,
		hotkeys: [{ hotkey: "A" }],
	});

	const rubCommand = useCommand(() => run(operations.rub), {
		enabled: !!operations.rub,
		commandPalette: operations.rub
			? { group: "Operation mode", label: operationLabel(operations.rub) }
			: undefined,
		shortcutsBar: operations.rub ? { label: operationLabel(operations.rub) } : undefined,
		hotkeys: [{ hotkey: "Mod+V", ignoreInputs: true }],
	});

	const moveBelowCommand = useCommand(() => run(operations.moveBelow), {
		enabled: !!operations.moveBelow,
		commandPalette: operations.moveBelow
			? { group: "Operation mode", label: operationLabel(operations.moveBelow) }
			: undefined,
		shortcutsBar: operations.moveBelow
			? { label: operationLabel(operations.moveBelow) }
			: undefined,
		hotkeys: [{ hotkey: "B" }],
	});

	const cancelCommand = useCommand(cancel, {
		commandPalette: { group: "Operation mode", label: "Cancel" },
		shortcutsBar: { label: "Cancel" },
		hotkeys: [{ hotkey: "Escape" }],
	});

	return (
		<>
			{operations.moveAbove && (
				<ShortcutButton
					className={uiStyles.button}
					hotkeys={moveAboveCommand.hotkeys}
					onClick={moveAboveCommand.commandFn}
				>
					{operationLabel(operations.moveAbove)}
				</ShortcutButton>
			)}
			{operations.rub && (
				<ShortcutButton
					className={uiStyles.button}
					hotkeys={rubCommand.hotkeys}
					onClick={rubCommand.commandFn}
				>
					{operationLabel(operations.rub)}
				</ShortcutButton>
			)}
			{operations.moveBelow && (
				<ShortcutButton
					className={uiStyles.button}
					hotkeys={moveBelowCommand.hotkeys}
					onClick={moveBelowCommand.commandFn}
				>
					{operationLabel(operations.moveBelow)}
				</ShortcutButton>
			)}
			<ShortcutButton
				className={uiStyles.button}
				hotkeys={cancelCommand.hotkeys}
				onClick={cancelCommand.commandFn}
			>
				Cancel
			</ShortcutButton>
		</>
	);
};

export const OperationTooltip: FC<
	{
		projectId: string;
		operand: Operand;
		operationMode: OperationMode | null;
		isActive: boolean;
	} & useRender.ComponentProps<"div">
> = ({ projectId, operand, operationMode, isActive, render, ...props }) => {
	const tooltip =
		isActive && !!operationMode
			? Match.value(operationMode).pipe(
					Match.tags({
						DragAndDrop: ({ operationType }) => {
							const operation = getOperation({
								source: operationMode.source,
								target: operand,
								operationType,
							});
							if (!operation) return null;

							return <>{operationLabel(operation)}</>;
						},
						Cut: ({ source }) => (
							<>
								{operandEquals(operationMode.source, operand) && <>Select a target</>}
								<CutOperationControls
									projectId={projectId}
									operations={getOperations(source, operand)}
								/>
							</>
						),
					}),
					Match.orElse(() => {
						const operation = getOperation({
							source: operationMode.source,
							target: operand,
							operationType: operationModeToOperationType(operationMode),
						});
						return (
							<>
								{operandEquals(operationMode.source, operand) && <>Select a target</>}
								<OperationModeControls projectId={projectId} operation={operation} />
							</>
						);
					}),
				)
			: null;

	const trigger = useRender({ render, props });

	const isDragAndDrop =
		!!operationMode &&
		Match.value(operationMode).pipe(
			Match.tags({ DragAndDrop: () => true }),
			Match.orElse(() => false),
		);

	return (
		<Tooltip.Root
			open={!!tooltip}
			disableHoverablePopup={isDragAndDrop}
			onOpenChange={(_open, eventDetails) => {
				eventDetails.allowPropagation();
			}}
		>
			<Tooltip.Trigger render={trigger} />
			<Tooltip.Portal>
				<Tooltip.Positioner sideOffset={8} side="right">
					<Tooltip.Popup className={classes(uiStyles.popup, uiStyles.tooltip, styles.popup)}>
						{tooltip}
					</Tooltip.Popup>
				</Tooltip.Positioner>
			</Tooltip.Portal>
		</Tooltip.Root>
	);
};
