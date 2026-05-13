import { createContext, FC, ReactNode, useContext } from "react";
import { createPortal } from "react-dom";

export const TopBarActionsElementContext = createContext<HTMLElement | null>(null);

export const TopBarActionsPortal: FC<{ children: ReactNode }> = ({ children }) => {
	const element = useContext(TopBarActionsElementContext);
	if (!element) return null;

	return createPortal(children, element);
};
