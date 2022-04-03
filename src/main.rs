mod version_operations;

use std::{error::Error, str::FromStr};

use console::{Style, Term};
use dialoguer::{theme::ColorfulTheme, Confirm, Editor, Input, Select};
use git2::{DescribeFormatOptions, DescribeOptions, Repository};
use semver::Version;
use strum::VariantNames;
use substring::Substring;
use version_operations::{MutVersion, SubVersion};

fn main() {
    tagger().unwrap();
}

fn tagger() -> Result<(), Box<dyn Error>> {
    let path = std::env::current_dir()?;
    let repo = Repository::open(path)?;

    // Get whether on main branch
    let head = &repo.head()?;
    assert!(head.is_branch());
    let head_commit = repo.find_object(head.target().unwrap(), None)?;
    let is_main = match head.name().unwrap() {
        "refs/heads/main" | "refs/heads/master" => true,
        _ => false,
    };

    // Get latest tags
    let (latest_version, latest_pre, all_pre) = get_latest_tags(&repo, &is_main)?;
    print_summary(&latest_version, &latest_pre, &all_pre);

    let next_tag_proposal = match is_main {
        true => prompt_increment(&latest_version),
        false => match latest_pre {
            Some(version) => Some(version.clone().increment_pretag(1)),
            None => prompt_increment(&latest_version).map(|x| x.increment_pretag(1)),
        },
    }
    .unwrap()
    .resolve_collision(&all_pre);

    // Determine new tag version and message
    let next_tag = prompt_next_tag(&next_tag_proposal).to_string();
    let message = get_message(&repo, latest_version).unwrap();

    // Create new tag
    let created_ref = repo.tag(&next_tag, &head_commit, &repo.signature()?, &message, false)?;
    let message_style = Style::new().italic();
    println!(
        "\nTag created:\n\n{:.7}\n{}\n",
        created_ref,
        message_style.apply_to(message)
    );

    // Push tag
    if Confirm::new().with_prompt("Push tag?").interact()? {
        let _success = repo
            .remotes()?
            .iter()
            .map(|name| repo.find_remote(name.unwrap()))
            .map(|remote| remote?.push(&vec![String::new(); 0], None));
        println!("Successfully pushed tag to origin.")
    };

    Ok(())
}

/// Obtains latest version, latest pre-version on current branch, 
/// and other pre-tags since the latest version
fn get_latest_tags(
    repo: &Repository,
    is_main: &bool,
) -> Result<(Version, Option<Version>, Vec<Version>), Box<dyn Error>> {
    let all_versions = repo
        .tag_names(None)?
        .iter()
        .filter_map(|name| parse_version(&name?))
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

    let latest_pre = match is_main {
        true => None,
        false => {
            let latest_pre_name = repo
                .describe(DescribeOptions::new().describe_tags())?
                .format(Some(DescribeFormatOptions::new().abbreviated_size(0)))?;
            parse_version(&latest_pre_name)
        }
    };
    Ok((latest_version, latest_pre, all_pre))
}

/// Prints a summary of current tags
fn print_summary(latest_version: &Version, latest_pre: &Option<Version>, all_pre: &Vec<Version>) {
    println!("\nLatest tags:");
    print_tag(&latest_version, "main");
    match latest_pre {
        Some(version) => print_tag(&version, "current branch"),
        None => {}
    }
    println!("\nAll current pre-tags:");
    all_pre.iter().for_each(|t| print_tag(&t, ""));
}

/// Prints a tag nicely
fn print_tag(version: &Version, annotation: &str) {
    let tag_style = Style::new().yellow().bold();
    println!(
        " {} {}",
        tag_style.apply_to(format!("v{: <10}", version)),
        annotation
    );
}

/// Parses tag into version string
fn parse_version(tag: &str) -> Option<Version> {
    let semver_str = tag.substring(1, tag.len());
    Version::parse(semver_str).ok()
}

/// Proposes new tag to user and prompts for confirmation
fn prompt_next_tag(proposal: &Version) -> Version {
    let input: String = Input::new()
        .with_prompt("\nNew tag")
        .default(proposal.to_string())
        .interact_text()
        .unwrap();

    Version::parse(&input).unwrap()
}

/// Determine message based on commit history and allow user to edit
fn get_message(repo: &Repository, latest_tag: Version) -> Option<String> {
    let mut revwalk = repo.revwalk().ok()?;
    revwalk.push_head().ok()?;
    revwalk
        .hide_ref(format!("refs/tags/{}", &latest_tag.to_string()).as_str())
        .unwrap();
    let commit_messages = revwalk
        .filter_map(|reference| repo.find_commit(reference.unwrap()).ok())
        .fold(String::from("release_notes:"), |acc, commit| {
            format!(
                "{}\n - {:.7} {}",
                acc,
                commit.id(),
                commit.summary().unwrap()
            )
        });
    Editor::new().edit(&commit_messages).ok()?
}

/// Prompt user which part of the version to increment
fn prompt_increment(version: &Version) -> Option<Version> {
    println!("\nCreate a new version with incremented:");
    let items = SubVersion::VARIANTS;
    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&items)
        .default(2)
        .interact_on_opt(&Term::stderr())
        .ok()?;
    selection.map(|i| {
        version
            .clone()
            .increment_version(SubVersion::from_str(items.get(i).unwrap()).unwrap())
    })
}
