import styles from "./Spinner.module.css";
import { classes } from "#ui/ui/classes.ts";
import { ComponentProps, FC } from "react";

export const Spinner: FC<ComponentProps<"span">> = ({ className, ...props }) => (
	<span {...props} className={classes(styles.spinner, className)} />
);
