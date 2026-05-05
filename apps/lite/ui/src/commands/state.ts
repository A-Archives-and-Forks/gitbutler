import { createSlice, type PayloadAction } from "@reduxjs/toolkit";
import type { CommandOptions } from "./manager.ts";

export type CommandRegistrationId = string;

type CommandsState = {
	registrations: Record<CommandRegistrationId, CommandOptions>;
};

const initialState: CommandsState = {
	registrations: {},
};

export const { actions: commandsActions, reducer: commandsReducer } = createSlice({
	name: "commands",
	initialState,
	reducers: {
		register: (
			state,
			action: PayloadAction<{ id: CommandRegistrationId; options: CommandOptions }>,
		) => {
			state.registrations[action.payload.id] = action.payload.options;
		},
		deregister: (state, action: PayloadAction<{ id: CommandRegistrationId }>) => {
			delete state.registrations[action.payload.id];
		},
	},
});
