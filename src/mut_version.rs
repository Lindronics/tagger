use anyhow::{Context, Result};
use semver::{Prerelease, Version};
use strum_macros::{EnumString, EnumVariantNames};
use substring::Substring;

#[derive(EnumString, EnumVariantNames)]
pub enum SubVersion {
    Major,
    Minor,
    Patch,
}

pub trait MutVersion {
    /// Increments the pre-version
    fn increment_prerelease(self, i: i32) -> Result<Self>
    where
        Self: Sized;

    /// Increments the version
    fn increment_version(self, part: SubVersion) -> Self;

    /// Sets the pre-version
    fn set_pretag(self, i: i32) -> Self;

    /// Increments pre-version until it does not conflict with existing releases
    fn resolve_collision(self, pre_tags: &[Version]) -> Result<Self>
    where
        Self: Sized;

    /// Prints version string including leading `v`
    fn print(&self) -> String;

    /// Returns git refname
    fn to_ref(&self) -> String;

    /// Parses tag with leading `v` from string
    fn parse_v(name: &str) -> Result<Self>
    where
        Self: Sized;
}

impl MutVersion for Version {
    fn increment_prerelease(self, i: i32) -> Result<Version> {
        let re = lazy_regex::regex!(r"pre(\d+)");
        let version_str = self.pre.as_str();

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
        Ok(self.set_pretag(new_pre_version))
    }

    fn set_pretag(mut self, i: i32) -> Version {
        self.pre = Prerelease::new(format!("pre{}", i).as_str())
            .expect("Could not set pre-version string");
        self
    }

    fn resolve_collision(self, pre_tags: &[Version]) -> Result<Self> {
        match pre_tags.contains(&self) {
            true => self.increment_prerelease(100)?.resolve_collision(pre_tags),
            false => Ok(self),
        }
    }

    fn increment_version(mut self, part: SubVersion) -> Self {
        match part {
            SubVersion::Major => {
                self.major += 1;
                self.minor = 0;
                self.patch = 0;
            }
            SubVersion::Minor => {
                self.minor += 1;
                self.patch = 0;
            }
            SubVersion::Patch => {
                self.patch += 1;
            }
        };
        self
    }

    fn print(&self) -> String {
        format!("v{}", self)
    }

    fn parse_v(name: &str) -> Result<Self> {
        let semver_str = name.substring(1, name.len());
        let version = Version::parse(semver_str)?;
        Ok(version)
    }

    fn to_ref(&self) -> String {
        format!("refs/tags/{}", &self.print())
    }
}
