import styles from "./Keys.module.css";
import { formatForDisplay, formatHotkeySequence, HotkeySequence } from "@tanstack/react-hotkeys";
import { FC } from "react";

type Props = {
	// We can't use the `Hotkey` type because it causes type errors in Storybook. 🤷‍♂️
	hotkey: string | HotkeySequence;
};

const formatKeys = (hotkey: string | HotkeySequence): string =>
	typeof hotkey === "string" ? formatForDisplay(hotkey) : formatHotkeySequence(hotkey);

export const Keys: FC<Props> = ({ hotkey }) => (
	<span className={styles.keys}>
		{formatKeys(hotkey)
			.split(" ")
			.map((key) => (
				<kbd key={key}>{key}</kbd>
			))}
	</span>
);
