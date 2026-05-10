import { persistSwallowGitHubOrgAuthErrors } from "$lib/config/config";
import {
	getTitleFromCommonErrorMessage,
	isBundlingError,
	isGitHubOrgAuthError,
	parseError,
	shouldIgnoreThisError,
} from "$lib/error/parser";
import { showToast, type Toast } from "$lib/notifications/toasts";

type ExtraAction = NonNullable<Toast["extraAction"]>;

export function showError(title: string, error: unknown, extraAction?: ExtraAction, id?: string) {
	const { name, message, code, description, ignored } = parseError(error);
	if (isBundlingError(message)) {
		console.warn(
			"You are likely experiencing a dev mode bundling error, " +
				"try disabling the cache from the network tab and " +
				"reload the page.",
		);
		return;
	}
	const commonErrorTitle = getTitleFromCommonErrorMessage(message);
	const actualTitle = name || commonErrorTitle || title;
	const shouldIgnoreThisSpecificError = shouldIgnoreThisError(actualTitle);

	if (!ignored && !shouldIgnoreThisSpecificError) {
		const offerToIgnore = isGitHubOrgAuthError(actualTitle);
		const actualExtraAction =
			extraAction ??
			(offerToIgnore
				? {
						label: "Don't show this again",
						onClick: () => {
							persistSwallowGitHubOrgAuthErrors(true);
						},
					}
				: undefined);

		const isWarn = code === "PreconditionFailed";

		showToast({
			id,
			title: actualTitle,
			message: description,
			error: message,
			style: isWarn ? "warning" : "danger",
			extraAction: actualExtraAction,
		});
	}
}
