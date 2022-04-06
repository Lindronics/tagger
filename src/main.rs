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

    // Get latest tags
    let (latest_version, latest_pre, all_pre) = get_latest_tags(&repo)?;
    print_summary(&latest_version, &latest_pre, &all_pre);

    // Generate proposal for new tag version
    let next_tag_proposal = match head.name().context("Could not get branch name")? {
        "refs/heads/main" | "refs/heads/master" => prompt_increment(&latest_version),
        _ => match latest_pre {
            Some(version) => Ok(version.clone()),
            None => prompt_increment(&latest_version),
        }
        .map(|version| version.increment_pretag(1)),
    }?
    .resolve_collision(&all_pre);

    // Determine new tag version and message
    let next_tag = prompt_next_tag(&next_tag_proposal)?;
    let message = get_message(&repo, latest_version)?;

    // Create new tag
    let _created_ref = repo.tag(
        &next_tag.print(),
        &head_commit,
        &repo.signature()?,
        &message,
        false,
    )?;

    let message_style = Style::new().italic();
    println!("\nTag created:\n");
    print_tag(&next_tag, "");
    println!("{}", message_style.apply_to(message));

    // Push tag
    if Confirm::new().with_prompt("\nPush tag?").interact()? {
        let push_output = Command::new("git").arg("push").arg("--tags").output()?;
        io::stdout().write_all(&push_output.stdout)?;
        assert!(push_output.status.success(), "Failed to push to remote");
    };

    Ok(())
}

/// Obtains latest version, latest pre-version on current branch,
/// and other pre-tags since the latest version
fn get_latest_tags(repo: &Repository) -> Result<(Version, Option<Version>, Vec<Version>)> {
    let all_versions = repo
        .tag_names(None)?
        .iter()
        .filter_map(|name| Version::parse_v(&name?).ok())
        .collect::<Vec<_>>();

    let latest_version = all_versions
        .iter()
        .filter(|version| version.pre.is_empty())
        .max()
        .map(|version| version.to_owned())
        .unwrap_or(Version::new(0, 0, 0));

    let all_pre = all_versions
        .into_iter()
        .filter(|version| !version.pre.is_empty())
        .filter(|version| version.gt(&latest_version))
        .collect::<Vec<_>>();

    let latest_pre = {
        let latest_pre_name = repo
            .describe(DescribeOptions::new().describe_tags())?
            .format(Some(DescribeFormatOptions::new().abbreviated_size(0)))?;
        Version::parse_v(&latest_pre_name).ok()
    };
    Ok((latest_version, latest_pre, all_pre))
}

/// Prints a summary of current tags
fn print_summary(latest_version: &Version, latest_pre: &Option<Version>, all_pre: &Vec<Version>) {
    println!("\nLatest tags:");
    print_tag(&latest_version, "main");
    if let Some(version) = latest_pre {
        print_tag(&version, "current branch")
    }
    println!("\nAll current pre-tags:");
    all_pre.iter().for_each(|version| print_tag(&version, ""));
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
        .with_prompt("\nNew tag")
        .default(proposal.print())
        .interact_text()?;

    let new_version = Version::parse_v(&input)?;
    Ok(new_version)
}

/// Determine message based on commit history and allow user to edit
fn get_message(repo: &Repository, latest_tag: Version) -> Result<String> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.hide_ref(&latest_tag.to_ref().as_str())?;
    let commit_messages = String::from("release_notes:\n")
        + &revwalk
            .filter_map(|reference| repo.find_commit(reference.ok()?).ok())
            .map(|commit| format!(" - {:.7} {}", commit.id(), commit.summary().unwrap_or("")))
            .collect::<Vec<String>>()
            .join("\n");
    let result = Editor::new().edit(&commit_messages)?;
    Ok(result.unwrap_or(String::new()))
}

/// Prompt user which part of the version to increment
fn prompt_increment(version: &Version) -> Result<Version> {
    println!("\nCreate a new version with incremented:");
    let items = SubVersion::VARIANTS;
    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&items)
        .default(2)
        .interact();
    let new_version = selection.map(|i| {
        version
            .to_owned()
            .increment_version(SubVersion::from_str(items.get(i).unwrap()).unwrap())
    })?;
    Ok(new_version)
}
