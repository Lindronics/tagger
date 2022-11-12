pub mod version;

use std::io::{self, Write};
use std::process::Command;
use std::str::FromStr;

use anyhow::Context;
use console::Style;
use dialoguer::Confirm;
use dialoguer::{theme::ColorfulTheme, Editor, Input, Select};
use git2::{DescribeFormatOptions, DescribeOptions, Repository};
use strum::VariantNames;
use version::{SubVersion, Version};

pub fn tagger(
    repo: &Repository,
    next_version: Option<Version>,
    interactive_editor: bool,
    prompt_push: bool,
) -> anyhow::Result<()> {
    println!("Fetching tags from remote...");
    let fetch_output = Command::new("git").arg("fetch").arg("--tags").output()?;
    io::stdout().write_all(&fetch_output.stdout)?;
    anyhow::ensure!(
        fetch_output.status.success(),
        "Failed to fetch tags: {}",
        String::from_utf8(fetch_output.stderr)?
    );

    // Check whether HEAD is a branch and get head commit
    let head = &repo.head()?;
    anyhow::ensure!(head.is_branch(), "HEAD is not a branch");
    let head_commit =
        repo.find_object(head.target().context("Could not get head commit")?, None)?;

    // Get latest tags and commits
    let all_tags = repo
        .tag_names(None)?
        .iter()
        .filter_map(|name| Version::parse(name?).ok())
        .collect::<Vec<_>>();

    let latest_release = all_tags
        .iter()
        .filter(|version| version.0.pre.is_empty())
        .max()
        .cloned();

    let latest_current_prerelease = repo
        .describe(DescribeOptions::new().describe_tags())
        .and_then(|description| {
            description.format(Some(DescribeFormatOptions::new().abbreviated_size(0)))
        })
        .ok()
        .and_then(|name: String| Version::parse(&name).ok())
        .filter(|version| !version.0.pre.is_empty());

    let commit_history = get_commit_history(repo, &all_tags)?;
    print_summary(
        &latest_release,
        &latest_current_prerelease,
        &all_tags,
        &commit_history,
    );

    // Determine new tag version
    let next_tag = match next_version {
        Some(version) => {
            if all_tags.contains(&version) {
                return Err(anyhow::format_err!("Version already exists"));
            }
            version
        }
        None => {
            // Generate proposal for new tag version
            let branch_name = head.name().context("Could not get branch name")?;
            let next_tag_proposal = match branch_name {
                "refs/heads/main" | "refs/heads/master" => prompt_increment(latest_release),
                _ => latest_current_prerelease
                    .unwrap_or(prompt_increment(latest_release)?)
                    .increment_prerelease(1),
            }?
            .resolve_collision(&all_tags)?;
            prompt_next_tag(&next_tag_proposal)?
        }
    };

    let mut message = format!(
        "release_notes:\n{}",
        commit_history.join("\n").replace(':', "")
    );
    if interactive_editor {
        message = Editor::new().edit(&message)?.unwrap_or_default();
    }

    // Create new tag
    let _created_ref = repo.tag(
        &next_tag.to_string(),
        &head_commit,
        &repo.signature()?,
        &message,
        false,
    )?;

    // Push tag
    if !prompt_push || Confirm::new().with_prompt("\nPush tag?").interact()? {
        let push_output = Command::new("git").arg("push").arg("--tags").output()?;
        io::stdout().write_all(&push_output.stdout)?;
        anyhow::ensure!(
            push_output.status.success(),
            "Failed to push to remote: {}",
            String::from_utf8(fetch_output.stderr)?
        );
    };

    Ok(())
}

/// Prints a summary of current tags
fn print_summary(
    latest_release: &Option<Version>,
    latest_prerelease: &Option<Version>,
    all_tags: &[Version],
    commit_messages: &[String],
) {
    let commit_message_style = Style::new().dim().italic();

    println!("\nLatest tags:");
    if let Some(version) = latest_release {
        print_tag(version, "main")
    }
    if let Some(version) = latest_prerelease {
        print_tag(version, "current branch")
    }

    println!("\nAll current prereleases:");
    for version in all_tags
        .iter()
        .filter(|version| !version.0.pre.is_empty())
        .filter(|&version| version.gt(&latest_release.to_owned().unwrap_or_default()))
    {
        print_tag(version, "")
    }

    println!("\nCommits since latest tag:");
    for message in commit_messages {
        println!("{}", commit_message_style.apply_to(message));
    }
    println!();
}

/// Prints a tag nicely
fn print_tag(version: &Version, annotation: &str) {
    let tag_style = Style::new().yellow().bold();
    println!(
        " {} {}",
        tag_style.apply_to(format!("{: <14}", version)),
        annotation
    );
}

/// Proposes new tag to user and prompts for confirmation
fn prompt_next_tag(proposal: &Version) -> anyhow::Result<Version> {
    let input: String = Input::new()
        .with_prompt("\nEnter new tag version")
        .default(proposal.to_string())
        .interact_text()?;
    Version::parse(&input)
}

/// Determine message based on commit history
fn get_commit_history(repo: &Repository, all_tags: &[Version]) -> anyhow::Result<Vec<String>> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    for tag in all_tags {
        revwalk.hide_ref(&tag.git_ref())?;
    }
    Ok(revwalk
        .filter_map(|reference| repo.find_commit(reference.ok()?).ok())
        .map(|commit| {
            format!(
                " - {:.7} {}",
                commit.id(),
                commit.summary().unwrap_or_default()
            )
        })
        .collect())
}

/// Prompt user which part of the version to increment
fn prompt_increment(version: Option<Version>) -> anyhow::Result<Version> {
    let items = SubVersion::VARIANTS;
    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(items)
        .with_prompt("Subversion to increment")
        .default(2)
        .interact()?;
    Ok(version
        .unwrap_or_default()
        .increment_version(SubVersion::from_str(
            items.get(selection).context("Invalid selection")?,
        )?))
}
