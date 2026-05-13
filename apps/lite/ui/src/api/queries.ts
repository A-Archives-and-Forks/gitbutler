import type {
	AbsorptionPlanParams,
	BranchDetailsParams,
	BranchDiffParams,
	CommitDetailsWithLineStatsParams,
	ListBranchesParams,
	TreeChangeDiffParams,
} from "#electron/ipc.ts";
import { queryOptions } from "@tanstack/react-query";

export enum QueryKey {
	BranchDetails = "branchDetails",
	BranchDiff = "branchDiff",
	ChangesInWorktree = "changesInWorktree",
	CommitDetailsWithLineStats = "commitDetailsWithLineStats",
	HeadInfo = "headInfo",
	Branches = "branches",
	Projects = "projects",
	TreeChangeDiffs = "treeChangeDiffs",
	AbsorptionPlan = "absorptionPlan",
}

export const branchDetailsQueryOptions = (params: BranchDetailsParams) =>
	queryOptions({
		queryKey: [QueryKey.BranchDetails, params],
		queryFn: () => window.lite.branchDetails(params),
	});

export const branchDiffQueryOptions = (params: BranchDiffParams) =>
	queryOptions({
		queryKey: [QueryKey.BranchDiff, params],
		queryFn: () => window.lite.branchDiff(params),
	});

export const changesInWorktreeQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: [QueryKey.ChangesInWorktree, projectId],
		queryFn: () => window.lite.changesInWorktree(projectId),
	});

export const commitDetailsWithLineStatsQueryOptions = (params: CommitDetailsWithLineStatsParams) =>
	queryOptions({
		queryKey: [QueryKey.CommitDetailsWithLineStats, params],
		queryFn: () => window.lite.commitDetailsWithLineStats(params),
	});

export const headInfoQueryOptions = (projectId: string) =>
	queryOptions({
		queryKey: [QueryKey.HeadInfo, projectId],
		queryFn: () => window.lite.headInfo(projectId),
	});

/** @public */
export const listBranchesQueryOptions = (params: ListBranchesParams) =>
	queryOptions({
		queryKey: [QueryKey.Branches, params],
		queryFn: () => window.lite.listBranches(params.projectId, params.filter),
	});

export const listProjectsQueryOptions = queryOptions({
	queryKey: [QueryKey.Projects],
	queryFn: () => window.lite.listProjects(),
});

export const treeChangeDiffsQueryOptions = ({ projectId, change }: TreeChangeDiffParams) =>
	queryOptions({
		queryKey: [QueryKey.TreeChangeDiffs, projectId, change],
		queryFn: () => window.lite.treeChangeDiffs({ projectId, change }),
	});

export const absorptionPlanQueryOptions = (params: AbsorptionPlanParams) =>
	queryOptions({
		queryKey: [QueryKey.AbsorptionPlan, params],
		queryFn: () => window.lite.absorptionPlan(params),
	});
