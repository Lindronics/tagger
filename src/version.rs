use std::fmt::Display;

use anyhow::{Context, Result};
use semver::{Prerelease, Version as Semver};
use strum_macros::{EnumString, EnumVariantNames};
use substring::Substring;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct Version(pub Semver);

#[derive(EnumString, EnumVariantNames)]
pub enum SubVersion {
    Major,
    Minor,
    Patch,
}

impl Version {
    pub fn increment_prerelease(self, i: i32) -> Result<Self> {
        let re = lazy_regex::regex!(r"pre(\d+)");
        let version_str = self.0.pre.as_str();

        let new_pre_version = match version_str {
            "" => 0,
            _ => {
                let cap = re
                    .captures(version_str)
                    .context("Failed to parse pre-version string")?;
                let pre_tag_version: i32 = cap[1].parse()?;
                pre_tag_version + i
            }
        };
        Ok(self.set_prerelease(new_pre_version))
    }

    pub fn set_prerelease(mut self, i: i32) -> Self {
        self.0.pre = Prerelease::new(format!("pre{}", i).as_str())
            .expect("Could not set pre-version string");
        self
    }

    pub fn resolve_collision(self, pre_tags: &[Self]) -> Result<Self> {
        match pre_tags.contains(&self) {
            true => self.increment_prerelease(100)?.resolve_collision(pre_tags),
            false => Ok(self),
        }
    }

    pub fn increment_version(mut self, part: SubVersion) -> Self {
        match part {
            SubVersion::Major => {
                self.0.major += 1;
                self.0.minor = 0;
                self.0.patch = 0;
            }
            SubVersion::Minor => {
                self.0.minor += 1;
                self.0.patch = 0;
            }
            SubVersion::Patch => {
                self.0.patch += 1;
            }
        };
        self
    }

    pub fn parse(name: &str) -> Result<Self> {
        let semver_str = name.substring(1, name.len());
        let version = Version(Semver::parse(semver_str)?);
        Ok(version)
    }

    pub fn git_ref(&self) -> String {
        format!("refs/tags/{}", &self)
    }
}

impl Default for Version {
    fn default() -> Self {
        Self(Semver::new(0, 1, 0))
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("v{}", self.0))
    }
}
