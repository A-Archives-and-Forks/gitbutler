import {
	normalizeRegisterableHotkey,
	type Hotkey,
	type RegisterableHotkey,
} from "@tanstack/react-hotkeys";

export type CommandGroup =
	| "Branch"
	| "Branches"
	| "Changes file"
	| "Changes"
	| "Commit file"
	| "Commit"
	| "Details"
	| "Files"
	| "Global"
	| "Outline"
	| "Operation mode"
	| "Panels"
	| "Rename branch"
	| "Reword commit"
	| "Stack";

declare module "@tanstack/react-hotkeys" {
	interface HotkeyMeta {
		group: CommandGroup;
	}
}

type HotkeySegment<T extends string> = T extends `${infer Head}+${infer Tail}`
	? Head | HotkeySegment<Tail>
	: T;

const electronAcceleratorKeys: Partial<Record<HotkeySegment<Hotkey>, string>> = {
	Alt: "Alt",
	ArrowDown: "Down",
	ArrowLeft: "Left",
	ArrowRight: "Right",
	ArrowUp: "Up",
	Backspace: "Backspace",
	Control: "Control",
	Delete: "Delete",
	End: "End",
	Escape: "Esc",
	Enter: "Enter",
	Home: "Home",
	Meta: "Command",
	Mod: "CommandOrControl",
	PageDown: "PageDown",
	PageUp: "PageUp",
	Shift: "Shift",
	Space: "Space",
	Tab: "Tab",
};

export const toElectronAccelerator = (hotkey: RegisterableHotkey): string | undefined => {
	const accelerator = normalizeRegisterableHotkey(hotkey)
		.split("+")
		.map((part) => electronAcceleratorKeys[part as HotkeySegment<Hotkey>] ?? part)
		.join("+");

	return accelerator.length > 0 ? accelerator : undefined;
};
