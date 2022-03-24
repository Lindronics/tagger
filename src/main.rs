use std::{collections::HashSet, iter::FromIterator};

use ansi_term::Colour::Yellow;
use console::Term;
use dialoguer::{theme::ColorfulTheme, Editor, Select, Input};
use git2::{Error, Oid, Repository, Tag};
use regex::Regex;
use semver::{BuildMetadata, Prerelease, Version};
use strum_macros::EnumIter;
use substring::Substring;

struct TagVersion<'a> {
    tag: Tag<'a>,
    version: Version,
}

#[derive(Debug, EnumIter)]
enum VersionLevel {
    Major = 0,
    Minor = 1,
    Patch = 2,
}

fn main() {
    let path = "/Users/lindronics/workspace/tests/tag_test";
    let repo = Repository::open(path).unwrap();

    get_latest_tags(&repo)
}

fn get_latest_tags(repo: &Repository) {
    let revwalk = get_branch_commits(&repo).unwrap();
    let current_branch_commits = HashSet::<Oid>::from_iter(revwalk.filter_map(Result::ok));

    let head = &repo.head().unwrap();
    if head.is_branch() {
        println!("Current branch: {}", head.name().unwrap())
    }

    let tag_names = repo.tag_names(None).unwrap();

    let versions = tag_names
        .iter()
        .filter_map(|name| name)
        .filter_map(|name| get_tag_version(&repo, name))
        .collect::<Vec<_>>();

    let latest_release = &versions
        .iter()
        .filter(|v| v.version.pre.is_empty())
        .map(|v| v.version.clone())
        .max()
        .unwrap_or(Version::new(0, 0, 0));

    let pre_tags = &versions
        .iter()
        .filter(|v| !v.version.pre.is_empty())
        .filter(|v| v.version.gt(latest_release))
        .collect::<Vec<_>>();

    let current_branch_pre_tag = pre_tags
        .iter()
        .filter(|v| current_branch_commits.contains(&v.tag.target().unwrap().id()))
        .max_by(|x, y| x.version.cmp(&y.version));

    println!("\nLatest tags:");
    print_tag(&latest_release, "main");
    current_branch_pre_tag.map(|v| print_tag(&v.version, "current branch"));

    println!("\nOther branches:");
    pre_tags
        .iter()
        .for_each(|t| println!(" {}", Yellow.paint(format!("v{}", &t.version.to_string()))));

    let next_tag_proposal = get_next_tag(
        latest_release,
        current_branch_pre_tag.map(|v| &v.version),
        pre_tags,
        head.name().unwrap() == "refs/heads/main",
    ).unwrap();

    let input: String = Input::new()
        .with_prompt("New tag")
        .default(next_tag_proposal.to_string())
        .interact_text().unwrap();

    let next_tag = Version::parse(&input).unwrap();

    // if let Some(rv) = Editor::new().edit("Enter a commit message").unwrap() {
    //     println!("Your message:");
    //     println!("{}", rv);
    // } else {
    //     println!("Abort!");
    // }
}

fn print_tag(version: &Version, annotation: &str) {
    println!(
        " {} - {}",
        Yellow.paint(format!("v{: <10}", version)),
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

fn get_next_tag(
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

fn prompt_increment(version: &Version) -> Option<Version> {
    let items = vec!["major", "minor", "patch"];
    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&items)
        .default(0)
        .interact_on_opt(&Term::stderr())
        .ok()?;
    match selection {
        Some(index) => match index {
            0 => Some(increment_version(version, VersionLevel::Major)),
            1 => Some(increment_version(version, VersionLevel::Minor)),
            _ => Some(increment_version(version, VersionLevel::Patch)),
        },
        None => None,
    }
}

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

fn increment_version(version: &Version, level: VersionLevel) -> Version {
    match level {
        VersionLevel::Major => Version::new(version.major + 1, 0, 0),
        VersionLevel::Minor => Version::new(version.major, version.minor + 1, 0),
        VersionLevel::Patch => Version::new(version.major, version.minor, version.patch + 1),
    }
}
