use std::{collections::HashSet, iter::FromIterator};

use console::{Style, Term};
use dialoguer::{theme::ColorfulTheme, Confirm, Editor, Input, Select};
use git2::{DescribeFormatOptions, DescribeOptions, Error, Oid, Repository, Tag};
use regex::Regex;
use semver::{BuildMetadata, Prerelease, Version};
use substring::Substring;

struct TagVersion<'a> {
    tag: Tag<'a>,
    version: Version,
}

fn main() {
    let path = "/Users/lindronics/workspace/tests/tag_test";
    let repo = Repository::open(path).unwrap();

    get_latest_tags(&repo)
}

fn get_latest_tags(repo: &Repository) {
    let revwalk = get_branch_commits(&repo).unwrap().filter_map(Result::ok);
    let current_branch_commits = HashSet::<Oid>::from_iter(revwalk);

    let head = &repo.head().unwrap();
    if head.is_branch() {
        println!("Current branch: {}", head.name().unwrap())
    }

    let tag_names = repo.tag_names(None).unwrap();

    let all_tags = tag_names
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
        .iter()
        .filter(|v| !v.version.pre.is_empty())
        .filter(|v| v.version.gt(latest_main_tag))
        .collect::<Vec<_>>();

    let latest_branch_pre_tag = all_pre_tags
        .iter()
        .filter(|v| current_branch_commits.contains(&v.tag.target().unwrap().id()))
        .max_by(|x, y| x.version.cmp(&y.version));

    println!("\nLatest tags:");
    print_tag(&latest_main_tag, "main");
    latest_branch_pre_tag.map(|v| print_tag(&v.version, "current branch"));

    println!("\nOther branches:");
    all_pre_tags.iter().for_each(|t| print_tag(&t.version, ""));

    let next_tag_proposal = get_next_tag_proposal(
        latest_main_tag,
        latest_branch_pre_tag.map(|v| &v.version),
        all_pre_tags,
        head.name().unwrap() == "refs/heads/main",
    )
    .unwrap();

    let next_tag = prompt_next_tag(&next_tag_proposal).to_string();
    let message = get_message(&repo).unwrap();

    let x = repo.head().unwrap().target().unwrap();
    let obj = repo.find_object(x, None).unwrap();

    let res = repo
        .tag(&next_tag, &obj, &repo.signature().unwrap(), &message, false)
        .unwrap();
    let create = Confirm::new()
        .with_prompt("Create tag?")
        .interact()
        .unwrap();

    if create {
        let success = repo
            .remotes()
            .unwrap()
            .iter()
            .map(|name| repo.find_remote(name.unwrap()))
            .map(|remote| remote.unwrap().push(&vec![String::new(); 0], None));
    }
}

// Print a tag nicely
fn print_tag(version: &Version, annotation: &str) {
    let tag_style = Style::new().yellow().bold();
    println!(
        " {} - {}",
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
    let main = repo.find_branch("main", git2::BranchType::Local)?;
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
    pre_tags: &Vec<&TagVersion>,
    is_main: bool,
) -> Option<Version> {
    match is_main {
        true => prompt_increment(latest),
        false => match latest_current {
            Some(version) => Some(increment_pretag(version)),
            None => prompt_increment(latest).map(|x| increment_pretag(&x)),
        },
    }
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
    let items = vec!["major", "minor", "patch"];
    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&items)
        .default(2)
        .interact_on_opt(&Term::stderr())
        .ok()?;
    selection.map(|index| match index {
        0 => Version::new(version.major + 1, 0, 0),
        1 => Version::new(version.major, version.minor + 1, 0),
        _ => Version::new(version.major, version.minor, version.patch + 1),
    })
}

// Increment the -pre string of the version
fn increment_pretag(version: &Version) -> Version {
    let re = Regex::new(r"pre(\d+)").unwrap();
    let version_str = version.pre.as_str();

    let new_pre_version = match version_str {
        "" => 0,
        _ => {
            let cap = re.captures(&version_str).unwrap();
            let pre_tag_version: i32 = cap[1].parse().unwrap();
            pre_tag_version + 1
        }
    };
    Version {
        major: version.major,
        minor: version.minor,
        patch: version.patch,
        pre: Prerelease::new(&format!("pre{}", new_pre_version)).unwrap(),
        build: BuildMetadata::EMPTY,
    }
}
