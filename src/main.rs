mod version_operations;

use std::io::{self, Write};
use std::process::Command;
use std::str::FromStr;

use anyhow::{Context, Result};
use console::Style;
use dialoguer::Confirm;
use dialoguer::{theme::ColorfulTheme, Editor, Input, Select};
use git2::{DescribeFormatOptions, DescribeOptions, Repository};
use semver::Version;
use strum::VariantNames;
use version_operations::{MutVersion, SubVersion};

fn main() {
    tagger().unwrap();
}

fn tagger() -> Result<()> {
    let path = std::env::current_dir()?;
    let repo = Repository::open(path)?;

    println!("Fetching tags from remote...");
    let fetch_output = Command::new("git").arg("fetch").arg("--tags").output()?;
    io::stdout().write_all(&fetch_output.stdout)?;
    assert!(fetch_output.status.success(), "Failed to fetch tags");

    // Check whether HEAD is a branch and get head commit
    let head = &repo.head()?;
    assert!(head.is_branch(), "HEAD is not a branch");
    let head_commit =
        repo.find_object(head.target().context("Could not get head commit")?, None)?;

    // Get latest tags and commits
    let all_tags = repo
        .tag_names(None)?
        .iter()
        .filter_map(|name| Version::parse_v(name?).ok())
        .collect::<Vec<_>>();

    let latest_version = all_tags
        .iter()
        .filter(|version| version.pre.is_empty())
        .max()
        .map(|version| version.to_owned());

    let all_pre = all_tags
        .into_iter()
        .filter(|version| !version.pre.is_empty())
        .filter(|version| version.gt(&latest_version.to_owned().unwrap_or(Version::new(0, 0, 0))))
        .collect::<Vec<_>>();

    let latest_current_pre = repo
        .describe(DescribeOptions::new().describe_tags())
        .and_then(|describe| {
            describe.format(Some(DescribeFormatOptions::new().abbreviated_size(0)))
        })
        .ok()
        .and_then(|name: String| Version::parse_v(&name).ok())
        .filter(|version| !version.pre.is_empty());

    let commits = get_commits(&repo, &latest_version, &latest_current_pre)?;
    print_summary(&latest_version, &latest_current_pre, &all_pre, &commits);

    // Generate proposal for new tag version
    let next_tag_proposal = match head.name().context("Could not get branch name")? {
        "refs/heads/main" | "refs/heads/master" => prompt_increment(latest_version),
        _ => match latest_current_pre {
            Some(version) => Ok(version),
            None => prompt_increment(latest_version),
        }
        .map(|version| version.increment_pretag(1)),
    }?
    .resolve_collision(&all_pre);

    // Determine new tag version and message
    let next_tag = prompt_next_tag(&next_tag_proposal)?;
    let message = edit_message(&commits)?;

    // Create new tag
    let _created_ref = repo.tag(
        &next_tag.print(),
        &head_commit,
        &repo.signature()?,
        &message,
        false,
    )?;

    // Push tag
    if Confirm::new().with_prompt("\nPush tag?").interact()? {
        let push_output = Command::new("git").arg("push").arg("--tags").output()?;
        io::stdout().write_all(&push_output.stdout)?;
        assert!(push_output.status.success(), "Failed to push to remote");
    };

    Ok(())
}

/// Prints a summary of current tags
fn print_summary(
    latest_version: &Option<Version>,
    latest_pre: &Option<Version>,
    all_pre: &[Version],
    commit_messages: &[String],
) {
    let commit_message_style = Style::new().dim().italic();
    println!("\nLatest tags:");
    if let Some(version) = latest_version {
        print_tag(version, "main")
    }
    if let Some(version) = latest_pre {
        print_tag(version, "current branch")
    }
    println!("\nAll current pre-tags:");
    for version in all_pre {
        print_tag(version, "")
    }
    println!("\nCommits:");
    for message in commit_messages {
        println!("{}", commit_message_style.apply_to(message));
    }
}

/// Prints a tag nicely
fn print_tag(version: &Version, annotation: &str) {
    let tag_style = Style::new().yellow().bold();
    println!(
        " {} {}",
        tag_style.apply_to(format!("{: <14}", version.print())),
        annotation
    );
}

/// Proposes new tag to user and prompts for confirmation
fn prompt_next_tag(proposal: &Version) -> Result<Version> {
    let input: String = Input::new()
        .with_prompt("\nEnter new tag version")
        .default(proposal.print())
        .interact_text()?;
    Version::parse_v(&input)
}

/// Determine message based on commit history
fn get_commits(
    repo: &Repository,
    latest_version: &Option<Version>,
    latest_pre: &Option<Version>,
) -> Result<Vec<String>> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    if let Some(version) = latest_version {
        revwalk.hide_ref(&version.to_ref())?;
    }
    if let Some(version) = latest_pre {
        revwalk.hide_ref(&version.to_ref())?;
    }
    Ok(revwalk
        .filter_map(|reference| repo.find_commit(reference.ok()?).ok())
        .map(|commit| format!(" - {:.7} {}", commit.id(), commit.summary().unwrap_or("")))
        .collect::<Vec<String>>())
}

/// Open editor to allow editing tag message
fn edit_message(commits: &[String]) -> Result<String> {
    let message = String::from("release_notes:\n") + &commits.join("\n");
    let result = Editor::new().edit(&message)?;
    Ok(result.unwrap_or_default())
}

/// Prompt user which part of the version to increment
fn prompt_increment(version: Option<Version>) -> Result<Version> {
    let items = SubVersion::VARIANTS;
    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(items)
        .with_prompt("Subversion to increment")
        .default(2)
        .interact()?;
    Ok(version
        .unwrap_or(Version::new(0, 0, 0))
        .increment_version(SubVersion::from_str(
            items.get(selection).context("Invalid selection")?,
        )?))
}
