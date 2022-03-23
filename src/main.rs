use std::{collections::HashSet, iter::FromIterator};

use git2::{string_array::StringArray, Error, Oid, Repository, Tag};
use semver::Version;
use substring::Substring;

fn main() {
    let path = "/Users/lindronics/workspace/tests/tag_test";
    let repo = Repository::open(path).unwrap();

    let revwalk = get_branch_commits(&repo).unwrap();
    let current_branch_commits = HashSet::<Oid>::from_iter(revwalk.filter_map(Result::ok));
    // set.iter().for_each(|x| println!("-> {:?}", x));

    let tag_names = repo.tag_names(None).unwrap();
    let latest_release = get_versions(&repo, &tag_names)
        .filter(|v| v.pre == Default::default())
        .max()
        .unwrap_or_else(|| Version::new(0, 0, 0));

    let pre_tags = get_versions(&repo, &tag_names)
        .filter(|v| v.pre != Default::default())
        .filter(|v| v > &latest_release);

    println!("Latest release: {:?}", latest_release);
    pre_tags.for_each(|t| println!("{}", t.to_string()))
}

fn get_versions<'a, 'b>(
    repo: &'a Repository,
    tag_names: &'a StringArray,
) -> impl Iterator<Item = Version> + 'a {
    // let tag_names = repo.tag_names(None).unwrap();
    let x = tag_names
        .iter()
        .filter_map(move |x| x.and_then(|y| get_tag(&repo, y).ok()))
        .map(|tag| parse_version(&tag).unwrap());
    return x;
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
