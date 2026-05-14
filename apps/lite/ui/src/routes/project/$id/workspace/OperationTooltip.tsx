import { absorptionPlanQueryOptions } from "#ui/api/queries.ts";
import { classes } from "#ui/ui/classes.ts";
import {
	getOperations,
	operationLabel,
	useRunOperationMutationOptions,
	type OperationType,
	type OperationsByType,
} from "#ui/operations/operation.ts";
import { ShortcutButton } from "#ui/components/ShortcutButton.tsx";
import uiStyles from "#ui/ui/ui.module.css";
import { Tooltip, useRender } from "@base-ui/react";
import { Toggle } from "@base-ui/react/toggle";
import { ToggleGroup } from "@base-ui/react/toggle-group";
import { FC } from "react";
import styles from "./OperationTooltip.module.css";
import { Operand, operandEquals } from "#ui/operands.ts";
import { useAppDispatch } from "#ui/store.ts";
import { projectActions } from "#ui/projects/state.ts";
import { getTransferOperation, type OutlineMode } from "#ui/outline/mode.ts";
import { Match } from "effect";
import { useHotkeys } from "@tanstack/react-hotkeys";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { type AbsorptionTarget } from "@gitbutler/but-sdk";

const AbsorbControls: FC<{
	projectId: string;
	sourceTarget: AbsorptionTarget;
}> = ({ projectId, sourceTarget }) => {
	const dispatch = useAppDispatch();
	const queryClient = useQueryClient();
	const absorptionPlan = useQuery(absorptionPlanQueryOptions({ projectId, target: sourceTarget }));
	const canAbsorb =
		!absorptionPlan.isPending && !!absorptionPlan.data && absorptionPlan.data.length > 0;
	const absorbMutation = useMutation({
		mutationFn: () => {
			if (!absorptionPlan.data) return Promise.resolve(0);
			return window.lite.absorb({ projectId, absorptionPlan: absorptionPlan.data });
		},
		onSuccess: async () => {
			await queryClient.invalidateQueries();
		},
	});

	const confirm = () => {
		dispatch(projectActions.exitMode({ projectId }));

		absorbMutation.mutate();
	};

	const cancel = () => dispatch(projectActions.cancelMode({ projectId }));

	return (
		<>
			<ShortcutButton
				className={uiStyles.button}
				hotkey="Enter"
				hotkeyOptions={{
					enabled: canAbsorb,
					meta: { group: "Operation mode", name: "Confirm" },
				}}
				onClick={confirm}
				disabled={!canAbsorb}
			>
				Absorb
			</ShortcutButton>
			<ShortcutButton
				className={uiStyles.button}
				hotkey="Escape"
				hotkeyOptions={{ meta: { group: "Operation mode", name: "Cancel" } }}
				onClick={cancel}
			>
				Cancel
			</ShortcutButton>
		</>
	);
};

const TransferOperationControls: FC<{
	projectId: string;
	operations: OperationsByType;
	operationType: OperationType;
}> = ({ projectId, operations, operationType }) => {
	const dispatch = useAppDispatch();
	const { mutate: runOperation } = useMutation(useRunOperationMutationOptions());
	const operation = operations[operationType];

	const run = () => {
		dispatch(projectActions.exitMode({ projectId }));

		if (!operation) return;

		runOperation(operation);
	};

	const cancel = () => dispatch(projectActions.cancelMode({ projectId }));

	const setOperationType = (operationType: OperationType) =>
		dispatch(projectActions.updateTransferOperationType({ projectId, operationType }));

	useHotkeys([
		{
			hotkey: "Mod+V",
			callback: run,
			options: {
				conflictBehavior: "allow",
				enabled: !!operation,
				ignoreInputs: true,
				meta: {
					group: "Operation mode",
					name: operation ? operationLabel(operation) : "Confirm",
				},
			},
		},
	]);

	const onValueChange = (value: Array<string>) => {
		if (value.length === 0) return;
		const nextOperationType = value[0] as OperationType;

		setOperationType(nextOperationType);
	};

	return (
		<>
			<ToggleGroup
				aria-label="Operation type"
				value={[operationType]}
				onValueChange={onValueChange}
				className={styles.operationTypeToggleGroup}
				orientation="vertical"
			>
				<Toggle
					value={"moveAbove" satisfies OperationType}
					className={styles.operationTypeToggle}
					render={
						<ShortcutButton
							hotkey="A"
							hotkeyOptions={{
								enabled: !!operations.moveAbove,
								meta: {
									group: "Operation mode",
									name: operations.moveAbove
										? `Select ${operationLabel(operations.moveAbove)}`
										: "Select move above",
								},
							}}
						/>
					}
				>
					{operations.moveAbove ? operationLabel(operations.moveAbove) : "Move above"}
				</Toggle>
				<Toggle
					value={"rub" satisfies OperationType}
					className={styles.operationTypeToggle}
					render={
						<ShortcutButton
							hotkey="R"
							hotkeyOptions={{
								enabled: !!operations.rub,
								meta: {
									group: "Operation mode",
									name: operations.rub ? `Select ${operationLabel(operations.rub)}` : "Select rub",
								},
							}}
						/>
					}
				>
					{operations.rub ? operationLabel(operations.rub) : "Rub"}
				</Toggle>
				<Toggle
					value={"moveBelow" satisfies OperationType}
					className={styles.operationTypeToggle}
					render={
						<ShortcutButton
							hotkey="B"
							hotkeyOptions={{
								enabled: !!operations.moveBelow,
								meta: {
									group: "Operation mode",
									name: operations.moveBelow
										? `Select ${operationLabel(operations.moveBelow)}`
										: "Select move below",
								},
							}}
						/>
					}
				>
					{operations.moveBelow ? operationLabel(operations.moveBelow) : "Move below"}
				</Toggle>
			</ToggleGroup>
			<ShortcutButton
				className={uiStyles.button}
				hotkey="Enter"
				hotkeyOptions={{
					enabled: !!operation,
					meta: {
						group: "Operation mode",
						name: operation ? operationLabel(operation) : "Confirm",
					},
				}}
				onClick={run}
				disabled={!operation}
			>
				Confirm
			</ShortcutButton>
			<ShortcutButton
				className={uiStyles.button}
				hotkey="Escape"
				hotkeyOptions={{ meta: { group: "Operation mode", name: "Cancel" } }}
				onClick={cancel}
			>
				Cancel
			</ShortcutButton>
		</>
	);
};

export const OperationTooltip: FC<
	{
		projectId: string;
		target: Operand;
		outlineMode: OutlineMode;
		isActive: boolean;
	} & useRender.ComponentProps<"div">
> = ({ projectId, target, outlineMode, isActive, render, ...props }) => {
	const tooltip = isActive
		? Match.value(outlineMode).pipe(
				Match.tags({
					Absorb: ({ sourceTarget }) => (
						<AbsorbControls projectId={projectId} sourceTarget={sourceTarget} />
					),
					Transfer: ({ value: mode }) =>
						Match.value(mode).pipe(
							Match.tagsExhaustive({
								Pointer: (mode) => {
									const operation = getTransferOperation({ mode, target });
									if (!operation) return null;

									return <>{operationLabel(operation)}</>;
								},
								Keyboard: (mode) => (
									<>
										{operandEquals(mode.source, target) && <>Select a target</>}
										<TransferOperationControls
											projectId={projectId}
											operations={getOperations(mode.source, target)}
											operationType={mode.operationType}
										/>
									</>
								),
							}),
						),
				}),
				Match.orElse(() => null),
			)
		: null;

	const trigger = useRender({ render, props });

	const isPointerTransfer = Match.value(outlineMode).pipe(
		Match.when({ _tag: "Transfer", value: { _tag: "Pointer" } }, () => true),
		Match.orElse(() => false),
	);

	return (
		<Tooltip.Root
			open={!!tooltip}
			disableHoverablePopup={isPointerTransfer}
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
