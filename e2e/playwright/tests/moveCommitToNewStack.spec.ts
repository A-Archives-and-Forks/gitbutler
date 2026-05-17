import { getBaseURL, type GitButler, startGitButler } from "../src/setup.ts";
import { test } from "../src/test.ts";
import { dragAndDropByLocator, waitForTestId } from "../src/util.ts";
import { expect, type Page } from "@playwright/test";

let gitbutler: GitButler;

test.use({
	baseURL: getBaseURL(),
});

test.afterEach(async () => {
	await gitbutler?.destroy();
});

/**
 * Set up a workspace with branch2 (2 commits, modifies b_file) and
 * branch3 (2 commits, modifies c_file) applied.
 * These branches are independent of each other and of master's a_file,
 * so moving commits between stacks won't cause merge conflicts.
 */
async function setupWorkspace(page: Page, context: any, testInfo: any) {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript("project-with-stacks.sh");
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch2", "local-clone"]);
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch3", "local-clone"]);

	await page.goto("/");
	await waitForTestId(page, "workspace-view");

	const stacks = page.getByTestId("stack");
	await expect(stacks).toHaveCount(2);
}

test("move a commit to the new stack dropzone to create a new stack", async ({
	page,
	context,
}, testInfo) => {
	test.setTimeout(120_000);
	await setupWorkspace(page, context, testInfo);

	const stack2 = page
		.getByTestId("stack")
		.filter({ has: page.getByTestId("branch-header").filter({ hasText: "branch2" }) });

	// branch2 should have 2 commits
	const commits = stack2.getByTestId("commit-row");
	await expect(commits).toHaveCount(2);

	// Pick a commit to drag
	const commitToDrag = commits.filter({ hasText: "branch2: first commit" });
	await expect(commitToDrag).toBeVisible();

	// Drag the commit onto the new stack dropzone
	const stackDropzone = await waitForTestId(page, "stack-offlane-dropzone");
	await dragAndDropByLocator(page, commitToDrag, stackDropzone, {
		force: true,
		position: {
			x: 10,
			y: 10,
		},
	});

	// Should now have three stacks (branch2, branch3, and the new one)
	const stacks = page.getByTestId("stack");
	await expect(stacks).toHaveCount(3, { timeout: 15_000 });

	// The original stack should have 1 commit (one was moved)
	await expect(stack2.getByTestId("commit-row")).toHaveCount(1, { timeout: 15_000 });

	// The moved commit should no longer be in the original stack
	await expect(
		stack2.getByTestId("commit-row").filter({ hasText: "branch2: first commit" }),
	).toHaveCount(0);
});

test("move multiple selected commits to the new stack dropzone", async ({
	page,
	context,
}, testInfo) => {
	test.setTimeout(120_000);
	await setupWorkspace(page, context, testInfo);

	const stack3 = page
		.getByTestId("stack")
		.filter({ has: page.getByTestId("branch-header").filter({ hasText: "branch3" }) });

	const commits = stack3.getByTestId("commit-row");
	await expect(commits).toHaveCount(2);

	// Multi-select both commits
	const firstCommit = commits.filter({ hasText: "branch3: first commit" });
	const secondCommit = commits.filter({ hasText: "branch3: second commit" });

	await firstCommit.click();
	const modKey = process.platform === "darwin" ? "Meta" : "Control";
	await secondCommit.click({ modifiers: [modKey] });

	// Both should be selected
	await expect(firstCommit).toHaveClass(/\bselected\b/);
	await expect(secondCommit).toHaveClass(/\bselected\b/);

	// Drag onto the new stack dropzone
	const stackDropzone = await waitForTestId(page, "stack-offlane-dropzone");
	await dragAndDropByLocator(page, firstCommit, stackDropzone, {
		force: true,
		position: {
			x: 10,
			y: 10,
		},
	});

	// Should now have three stacks
	const stacks = page.getByTestId("stack");
	await expect(stacks).toHaveCount(3, { timeout: 15_000 });

	// The original branch3 stack should be empty (both commits were moved).
	// An empty branch shows no commit rows.
	await expect(stack3.getByTestId("commit-row")).toHaveCount(0, { timeout: 15_000 });
});
