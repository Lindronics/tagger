mod version_operations;

use std::{collections::HashSet, iter::FromIterator, str::FromStr};

use console::{Style, Term};
use dialoguer::{theme::ColorfulTheme, Confirm, Editor, Input, Select};
use git2::{Branch, DescribeFormatOptions, DescribeOptions, Error, Oid, Repository, Tag};
use semver::Version;
use strum::VariantNames;
use substring::Substring;
use version_operations::{MutVersion, SubVersion};

struct TagVersion<'a> {
    tag: Tag<'a>,
    version: Version,
}

fn main() {
    let path = "/Users/lindronics/workspace/tests/tag_test";
    // let path = std::env::current_dir().unwrap();
    let repo = Repository::open(path).unwrap();

    get_latest_tags(&repo)
}

fn get_latest_tags(repo: &Repository) {
    let revwalk = get_branch_commits(&repo).unwrap().filter_map(Result::ok);
    let current_branch_commits = HashSet::<Oid>::from_iter(revwalk);

    let head = &repo.head().unwrap();
    if !head.is_branch() {
        return;
    }
    let head_commit = repo.find_object(head.target().unwrap(), None).unwrap();

    let all_tags = repo
        .tag_names(None)
        .unwrap()
        .iter()
        .filter_map(|name| name)
        .filter_map(|name| get_tag_version(&repo, name))
        .collect::<Vec<_>>();

    let latest_main_tag = &all_tags
        .iter()
        .filter(|v| v.version.pre.is_empty())
        .map(|v| v.version.clone())
        .max()
        .unwrap_or(Version::new(0, 0, 0));

    let all_pre_tags = &all_tags
        .into_iter()
        .filter(|v| !v.version.pre.is_empty())
        .filter(|v| v.version.gt(latest_main_tag))
        .collect::<Vec<_>>();

    let latest_branch_pre_tag = all_pre_tags
        .iter()
        .filter(|v| current_branch_commits.contains(&v.tag.target().unwrap().id()))
        .map(|v| &v.version)
        .max();

    println!("\nLatest tags:");
    print_tag(&latest_main_tag, "main");
    latest_branch_pre_tag.map(|v| print_tag(&v, "current branch"));

    println!("\nOther branches:");
    all_pre_tags.iter().for_each(|t| print_tag(&t.version, ""));

    let next_tag_proposal = get_next_tag_proposal(
        latest_main_tag,
        latest_branch_pre_tag,
        &all_pre_tags
            .iter()
            .map(|v| v.version.clone())
            .collect::<Vec<_>>(),
        head.name().unwrap() == "refs/heads/main" || head.name().unwrap() == "refs/heads/master",
    )
    .unwrap();

    let next_tag = prompt_next_tag(&next_tag_proposal).to_string();
    let message = get_message(&repo).unwrap();

    let created_ref = repo
        .tag(
            &next_tag,
            &head_commit,
            &repo.signature().unwrap(),
            &message,
            false,
        )
        .unwrap();

    let message_style = Style::new().italic();
    println!(
        "\nTag created:\n\n{:.7}\n{}\n",
        created_ref,
        message_style.apply_to(message)
    );

    if Confirm::new().with_prompt("Push tag?").interact().unwrap() {
        let _success = repo
            .remotes()
            .unwrap()
            .iter()
            .map(|name| repo.find_remote(name.unwrap()))
            .map(|remote| remote.unwrap().push(&vec![String::new(); 0], None));
        println!("Successfully pushed tag to origin.")
    };
}

fn get_main_branch(repo: &Repository) -> Result<Branch, Error> {
    repo.find_branch("main", git2::BranchType::Local)
        .or(repo.find_branch("master", git2::BranchType::Local))
}

// Print a tag nicely
fn print_tag(version: &Version, annotation: &str) {
    let tag_style = Style::new().yellow().bold();
    println!(
        " {} {}",
        tag_style.apply_to(format!("v{: <10}", version)),
        annotation
    );
}

fn get_tag_version<'a>(repo: &'a Repository, tag_name: &str) -> Option<TagVersion<'a>> {
    let tag = get_tag(&repo, tag_name).ok()?;
    let version = parse_version(&tag)?;
    Some(TagVersion { tag, version })
}

// Get tag object from name
fn get_tag<'repo>(repo: &'repo Repository, tag_name: &str) -> Result<Tag<'repo>, Error> {
    let ref_name = format!("refs/tags/{}", tag_name);
    repo.find_reference(&ref_name).and_then(|x| x.peel_to_tag())
}

// Parses tag into version string
fn parse_version(tag: &Tag) -> Option<Version> {
    let version_substr = tag.name()?;
    let semver_str = version_substr.substring(1, version_substr.len());
    Version::parse(semver_str).ok()
}

// Get all commits on current branch
fn get_branch_commits(repo: &Repository) -> Result<git2::Revwalk, Error> {
    let main = get_main_branch(repo)?;
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.hide_ref(main.get().name().unwrap())?;
    Ok(revwalk)
}

// Proposes new tag to user and prompts for confirmation
fn prompt_next_tag(proposal: &Version) -> Version {
    let input: String = Input::new()
        .with_prompt("\nNew tag")
        .default(proposal.to_string())
        .interact_text()
        .unwrap();

    Version::parse(&input).unwrap()
}

// Determine new tag proposal based on tag history
fn get_next_tag_proposal(
    latest: &Version,
    latest_current: Option<&Version>,
    pre_tags: &Vec<Version>,
    is_main: bool,
) -> Option<Version> {
    match is_main {
        true => prompt_increment(latest),
        false => match latest_current {
            Some(version) => Some(version.clone().increment_pretag(1)),
            None => prompt_increment(latest).map(|x| x.increment_pretag(1)),
        },
    }
    .map(|version| version.resolve_collision(pre_tags))
}

// Determine message based on commit history and allow user to edit
fn get_message(repo: &Repository) -> Option<String> {
    let previous_tag = repo
        .describe(DescribeOptions::new().describe_tags())
        .ok()?
        .format(Some(DescribeFormatOptions::new().abbreviated_size(0)))
        .ok()?;

    let mut revwalk = repo.revwalk().unwrap();
    revwalk.push_head().unwrap();
    revwalk
        .hide_ref(format!("refs/tags/{}", &previous_tag).as_str())
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
    Editor::new().edit(&commit_messages).unwrap()
}

// Prompt user which part of the version to increment
fn prompt_increment(version: &Version) -> Option<Version> {
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
