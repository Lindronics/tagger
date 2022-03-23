use std::{collections::HashSet, iter::FromIterator};

use ansi_term::Colour::Yellow;
use git2::{Error, Oid, Repository, Tag};
use semver::Version;
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
    let revwalk = get_branch_commits(&repo).unwrap();
    let current_branch_commits = HashSet::<Oid>::from_iter(revwalk.filter_map(Result::ok));

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
    println!(
        " {} - (main)",
        Yellow.paint(format!("v{: <10}", &latest_release))
    );
    println!(
        " {} - (current branch)",
        Yellow.paint(format!(
            "v{: <10}",
            &current_branch_pre_tag.unwrap().version
        ))
    );
    println!("\nOther branches:");
    pre_tags
        .iter()
        .for_each(|t| println!(" {}", Yellow.paint(format!("v{}", &t.version.to_string()))));
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
