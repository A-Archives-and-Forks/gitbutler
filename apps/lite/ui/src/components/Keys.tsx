import styles from "./Keys.module.css";
import {
	formatForDisplay,
	formatHotkeySequence,
	Hotkey,
	HotkeySequence,
} from "@tanstack/react-hotkeys";
import { FC } from "react";

type Props = {
	hotkey: Hotkey | HotkeySequence;
};

const formatKeys = (hotkey: Hotkey | HotkeySequence): string =>
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
