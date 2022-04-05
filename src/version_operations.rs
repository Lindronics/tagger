use regex::Regex;
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
    /// Increment the pre-version
    fn increment_pretag(self, i: i32) -> Self;

    /// Set the pre string of the version
    fn increment_version(self, part: SubVersion) -> Self;

    /// Checks whether version already exists and increments pre-version if necessary
    fn set_pretag(self, i: i32) -> Self;

    /// Increments the selected subversion
    fn resolve_collision(self, pre_tags: &Vec<Version>) -> Self;

    /// Prints version string including leading `v`
    fn print(&self) -> String;

    /// Returns git refname
    fn to_ref(&self) -> String;

    /// Parses tag with leading `v` from string
    fn parse_v(name: &str) -> Option<Self>
    where
        Self: Sized;
}

impl MutVersion for Version {
    fn increment_pretag(self, i: i32) -> Version {
        let re = Regex::new(r"pre(\d+)").unwrap();
        let version_str = self.pre.as_str();

        let new_pre_version = match version_str {
            "" => 0,
            _ => {
                let cap = re.captures(&version_str).unwrap();
                let pre_tag_version: i32 = cap[1].parse().unwrap();
                pre_tag_version + i
            }
        };
        self.set_pretag(new_pre_version)
    }

    fn set_pretag(mut self, i: i32) -> Version {
        self.pre = Prerelease::new(format!("pre{}", i).as_str()).unwrap();
        self
    }

    fn resolve_collision(self, pre_tags: &Vec<Version>) -> Self {
        match pre_tags.contains(&self) {
            true => self.increment_pretag(100).resolve_collision(pre_tags),
            false => self,
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
        format!("v{}", self.to_string())
    }

    fn parse_v(name: &str) -> Option<Self> {
        let semver_str = name.substring(1, name.len());
        Version::parse(semver_str).ok()
    }

    fn to_ref(&self) -> String {
        format!("refs/tags/{}", &self.print())
    }
}
