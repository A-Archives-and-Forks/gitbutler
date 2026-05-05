import {
	type HotkeySequence,
	type UseHotkeySequenceOptions,
	type UseHotkeyOptions,
	type RegisterableHotkey,
	normalizeRegisterableHotkey,
	useHotkeys,
	UseHotkeyDefinition,
	useHotkeySequences,
	UseHotkeySequenceDefinition,
	Hotkey,
} from "@tanstack/react-hotkeys";
import { Context, createContext, useContext, useEffect, useId } from "react";
import type { CommandGroup } from "./groups";
import type { NativeMenuItem, NativeMenuItemData } from "#ui/native-menu.ts";
import { useAppDispatch, useAppSelector } from "#ui/store.ts";
import { commandsActions, CommandRegistrationId } from "./state";
import { Order } from "effect";
import { optionalOrder } from "#ui/lib/order.ts";

export type CommandLayer =
	| "global"
	| "selection-tree"
	| "selection"
	| "focused-selection-tree"
	| "focused-selection";

export const CommandLayerOrder: Order.Order<CommandLayer> = Order.mapInput(Order.number, (cl) => {
	switch (cl) {
		case "global":
			return 0;
		case "selection-tree":
			return 1;
		case "selection":
			return 2;
		case "focused-selection-tree":
			return 3;
		case "focused-selection":
			return 4;
	}
});

// consider if many of these could typically share a label
export type CommandOptions = {
	/** @default true */
	enabled?: boolean;
	layer: CommandLayer;
	commandPalette?: {
		group: CommandGroup;
		label: string;
		/** @default true */
		hotkeys?: boolean;
	};
	shortcutsBar?: {
		label: string;
	};
	contextMenu?: NativeMenuItemData;
	hotkeys?: Array<CommandHotkey | CommandHotkeySequence>;
};

type CommandHotkey = {
	hotkey: RegisterableHotkey;
} & Omit<
	UseHotkeyOptions,
	// Causes a type error with Immer.
	"target"
>;

type CommandHotkeySequence = {
	sequence: HotkeySequence;
} & Omit<
	UseHotkeySequenceOptions,
	// Causes a type error with Immer.
	"target"
>;

type CommandTrigger = "commandPalette" | "contextMenu" | "hotkey" | "ui";

export type CommandFn = (scenario: CommandTrigger) => void;

export const CommandFnContext: Context<Map<CommandRegistrationId, CommandFn> | null> =
	createContext<Map<CommandRegistrationId, CommandFn> | null>(null);

const sequenceKey = (s: HotkeySequence): string =>
	s
		.join("")
		// Disambiguate from non-sequenced hotkeys.
		.concat("_seq_");

const useMaxHotkeyLayers = (): Record<string, CommandLayer | undefined> => {
	const regs = useAppSelector((state) => state.commands.registrations);

	return Object.values(regs).reduce(
		(acc, val) => {
			if (!val.hotkeys) return acc;

			for (const hotkey of val.hotkeys) {
				const k =
					"sequence" in hotkey
						? sequenceKey(hotkey.sequence)
						: normalizeRegisterableHotkey(hotkey.hotkey);
				acc[k] = Order.max(optionalOrder(CommandLayerOrder))(acc[k], val.layer);
			}

			return acc;
		},
		{} as Record<string, CommandLayer | undefined>,
	);
};

type ResolvedCommand<F extends CommandFn, O extends CommandOptions> = {
	commandFn: F;
	contextMenu: O extends { contextMenu: NativeMenuItemData } ? NativeMenuItem : undefined;
	hotkeys: O extends { hotkeys: Array<CommandHotkey | CommandHotkeySequence> }
		? Array<Hotkey | HotkeySequence>
		: undefined;
};

// future: maybe add useCommands. and/or internal multi hotkeys like useHotkeys
// separating the function from options improves ability to memo by ref of options obj
/**
 * Hotkeys are automatically disabled when a layer of higher precedence is enabled with the same
 * keybind.
 */
export const useCommand = <F extends CommandFn, O extends CommandOptions>(
	commandFn: F,
	options: O,
): ResolvedCommand<F, O> => {
	const id = useId();
	// oxlint-disable-next-line typescript/no-non-null-assertion: Let it loudly fail.
	const cbmap = useContext(CommandFnContext)!;
	const dispatch = useAppDispatch();
	const maxKeybindLayers = useMaxHotkeyLayers();

	useEffect(() => {
		dispatch(commandsActions.register({ id, options }));

		return () => void dispatch(commandsActions.deregister({ id }));
	}, [dispatch, id, options]);

	useEffect(() => {
		cbmap.set(id, commandFn);

		return () => void cbmap.delete(id);
	}, [cbmap, id, commandFn]);

	const { hotkeyDefs, sequenceDefs, resolvedHotkeys } = (options.hotkeys ?? []).reduce(
		(acc, hk) => {
			const maxKeybindLayer =
				maxKeybindLayers[
					"sequence" in hk ? sequenceKey(hk.sequence) : normalizeRegisterableHotkey(hk.hotkey)
				];

			const defEnabled =
				options.layer === maxKeybindLayer && options.enabled !== false && hk.enabled !== false;

			if (defEnabled)
				acc.resolvedHotkeys.push(
					"sequence" in hk ? hk.sequence : normalizeRegisterableHotkey(hk.hotkey),
				);

			const def: UseHotkeyDefinition | UseHotkeySequenceDefinition = {
				// We only want to be warned if two conflicting hotkeys are enabled at the same time. NB we
				// must therefore be wary of which keys we use directly with useHotkey(s).
				callback: () => commandFn("hotkey"),
				options: {
					enabled: defEnabled,
					conflictBehavior: defEnabled ? "warn" : "allow",
					// Allow overriding any of our default behavior if you really want to...
					...hk,
				},
				// ...at both layers, since the shapes don't align. Irrelevant keys are ignored.
				...hk,
			};

			if ("sequence" in hk) acc.sequenceDefs.push(def as UseHotkeySequenceDefinition);
			else acc.hotkeyDefs.push(def as UseHotkeyDefinition);

			return acc;
		},
		{
			hotkeyDefs: [] as Array<UseHotkeyDefinition>,
			sequenceDefs: [] as Array<UseHotkeySequenceDefinition>,
			resolvedHotkeys: [] as Array<Hotkey | HotkeySequence>,
		},
	);

	useHotkeys(hotkeyDefs);
	useHotkeySequences(sequenceDefs);

	return {
		commandFn,
		hotkeys: resolvedHotkeys.length > 0 ? resolvedHotkeys : undefined,
		contextMenu: options.contextMenu
			? {
					enabled: options.enabled !== false,
					onSelect: () => commandFn("contextMenu"),
					...options.contextMenu,
					_tag: "Item",
				}
			: undefined,
	} as ResolvedCommand<F, O>;
};

export const useCommandFn = (): ((id: CommandRegistrationId) => CommandFn | undefined) => {
	// oxlint-disable-next-line typescript/no-non-null-assertion: Let it loudly fail.
	const map = useContext(CommandFnContext)!;
	return (id) => map.get(id);
};
