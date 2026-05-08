import { GIT_CONFIG_GLOBAL } from "../src/env.ts";
import { writeToFile } from "../src/file.ts";
import { getBaseURL, type GitButler, startGitButler } from "../src/setup.ts";
import { test } from "../src/test.ts";
import {
	clickByTestId,
	fillByTestId,
	getByTestId,
	mockPickDirectory,
	textEditorFillByTestId,
	waitForTestId,
} from "../src/util.ts";
import { expect } from "@playwright/test";

let gitbutler: GitButler;

test.use({
	baseURL: getBaseURL(),
});

test.afterEach(async () => {
	await gitbutler?.destroy();
});

test("should start the application and be able to commit", async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	const projectPath = gitbutler.pathInWorkdir("local-clone/");

	await gitbutler.runScript("setup-empty-project.sh");

	await page.goto("/");
	const onboardingPage = getByTestId(page, "onboarding-page");
	await onboardingPage.waitFor();

	clickByTestId(page, "analytics-continue");

	// Add a local project
	await mockPickDirectory(page, projectPath);
	await clickByTestId(page, "add-local-project");

	// Should see the set target page
	await waitForTestId(page, "project-setup-page");

	clickByTestId(page, "set-base-branch");

	// Should load the workspace directly after setting base branch
	await waitForTestId(page, "workspace-view");

	// Let's write some files
	const filePath = gitbutler.pathInWorkdir("local-clone/test-file.txt");
	writeToFile(filePath, "This is supper important content");

	// Should see the uncommitted changes list
	await waitForTestId(page, "uncommitted-changes-file-list");
	const files = getByTestId(page, "file-list-item");

	await expect(files).toHaveCount(1);
	await expect(files.first()).toHaveText("test-file.txt");

	// Click the commit button
	await clickByTestId(page, "commit-to-new-branch-button");

	// Should see the commit drawer
	await waitForTestId(page, "new-commit-view");

	const newCommitMessage = "New commit message";
	const newCommitMessageBody = "This is the body of the commit message.";
	// Write a commit message
	await fillByTestId(page, "commit-drawer-title-input", newCommitMessage);
	await textEditorFillByTestId(page, "commit-drawer-description-input", newCommitMessageBody);

	// Click the commit button
	await clickByTestId(page, "commit-drawer-action-button");

	const commitRows = getByTestId(page, "commit-row");
	await expect(commitRows).toHaveCount(1);
	await expect(commitRows.first()).toHaveText(newCommitMessage);
});

test("no author setup - should start the application and be able to commit", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	const otherGitConfig = testInfo.outputPath("config/gitconfig");

	gitbutler = await startGitButler(workdir, configdir, context, {
		GIT_CONFIG_GLOBAL: otherGitConfig,
	});

	const projectPath = gitbutler.pathInWorkdir("local-clone/");

	await gitbutler.runScript("setup-empty-project.sh", undefined, {
		// Use the right config to create the setup
		GIT_CONFIG_GLOBAL,
	});

	await page.goto("/");
	const onboardingPage = getByTestId(page, "onboarding-page");
	await onboardingPage.waitFor();

	clickByTestId(page, "analytics-continue");

	// Add a local project
	await mockPickDirectory(page, projectPath);
	await clickByTestId(page, "add-local-project");

	// Should see the set target page
	await waitForTestId(page, "project-setup-page");

	clickByTestId(page, "set-base-branch");

	// Should load the workspace directly after setting base branch
	await waitForTestId(page, "workspace-view");

	// Should see the author missing modal
	await waitForTestId(page, "global-modal-author-missing");

	await fillByTestId(page, "global-modal-author-missing-name-input", "Test User");
	await fillByTestId(page, "global-modal-author-missing-email-input", "test@example.com");
	await clickByTestId(page, "global-modal-author-missing-action-button");

	// Wait for the author-missing modal to close before interacting with the page
	await expect(getByTestId(page, "global-modal-author-missing")).toBeHidden();

	// Let's write some files
	const filePath = gitbutler.pathInWorkdir("local-clone/test-file.txt");
	writeToFile(filePath, "This is supper important content");

	// Should see the uncommitted changes list
	await waitForTestId(page, "uncommitted-changes-file-list");
	const files = getByTestId(page, "file-list-item");

	await expect(files).toHaveCount(1);
	await expect(files.first()).toHaveText("test-file.txt");

	// Click the commit button
	await clickByTestId(page, "commit-to-new-branch-button");

	// Should see the commit drawer
	await waitForTestId(page, "new-commit-view");

	const newCommitMessage = "New commit message";
	const newCommitMessageBody = "This is the body of the commit message.";
	// Write a commit message
	await fillByTestId(page, "commit-drawer-title-input", newCommitMessage);
	await textEditorFillByTestId(page, "commit-drawer-description-input", newCommitMessageBody);

	// Click the commit button
	await clickByTestId(page, "commit-drawer-action-button");

	const commitRows = getByTestId(page, "commit-row");
	await expect(commitRows).toHaveCount(1);
	await expect(commitRows.first()).toHaveText(newCommitMessage);
});
