use anyhow::Context;
use clap::Parser;
use git2::Repository;
use semver::Version;
use tagger::{mut_version::MutVersion, tagger};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Optional name of the new tag to create.
    /// If not supplied, will determine interactively.
    name: Option<String>,

    /// If this flag is set, allow editing the
    /// message in an interactive editor
    /// before creating the tag.
    #[clap(short, long)]
    interactive_editor: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let path = std::env::current_dir()?;
    let repo = Repository::open(path)?;

    let next_version = match args.name {
        Some(name) => Some(Version::parse_v(&name).context("Not a valid version string")?),
        None => None,
    };

    tagger(&repo, next_version, args.interactive_editor)
}
