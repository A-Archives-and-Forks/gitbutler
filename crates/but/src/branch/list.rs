use colored::Colorize;
use gitbutler_branch_actions::BranchListingFilter;
use gitbutler_project::Project;

pub async fn list(project: &Project, local: bool) -> Result<(), anyhow::Error> {
    let filter = if local {
        Some(BranchListingFilter {
            local: Some(true),
            ..Default::default()
        })
    } else {
        None
    };

    let branch_review_map = crate::forge::review::get_review_map(project).await?;

    let applied_stacks =
        but_api::workspace::stacks(project.id, Some(but_workspace::StacksFilter::InWorkspace))?;
    print_applied_branches(&applied_stacks, &branch_review_map);
    let branches = but_api::virtual_branches::list_branches(project.id, filter)?;
    let (branches, remote_only_branches): (Vec<_>, Vec<_>) =
        branches.into_iter().partition(|b| b.has_local);
    for branch in branches {
        // Skip branches that are part of applied stacks
        if let Some(stack_ref) = branch.stack
            && applied_stacks.iter().any(|s| s.id == Some(stack_ref.id))
        {
            continue;
        }

        let reviews =
            crate::forge::review::get_review_numbers(&branch.name.to_string(), &branch_review_map);

        println!("{}{}", branch.name, reviews);
    }

    for branch in remote_only_branches {
        let reviews =
            crate::forge::review::get_review_numbers(&branch.name.to_string(), &branch_review_map);
        println!("{} {}{}", "(remote)".dimmed(), branch.name, reviews);
    }
    Ok(())
}

fn print_applied_branches(
    applied_stacks: &[but_workspace::ui::StackEntry],
    branch_review_map: &std::collections::HashMap<
        String,
        Vec<gitbutler_forge::review::ForgeReview>,
    >,
) {
    for stack in applied_stacks {
        let first_branch = stack.heads.first();
        let last_branch = stack.heads.last();
        for branch in &stack.heads {
            let is_single_branch = stack.heads.len() == 1;
            if is_single_branch {
                let branch_entry = format!("* {}", branch.name);
                let reviews = crate::forge::review::get_review_numbers(
                    &branch.name.to_string(),
                    branch_review_map,
                );
                println!("{}{}", branch_entry.green(), reviews);
                continue;
            }

            let Some(first_branch) = first_branch else {
                continue;
            };

            let Some(last_branch) = last_branch else {
                continue;
            };

            let branch_entry = if branch.name == first_branch.name {
                format!("*- {}", branch.name)
            } else if branch.name == last_branch.name {
                format!("└─ {}", branch.name)
            } else {
                format!("├─ {}", branch.name)
            };

            let reviews = crate::forge::review::get_review_numbers(
                &branch.name.to_string(),
                branch_review_map,
            );

            println!("{}{}", branch_entry.green(), reviews);
        }
    }
}
