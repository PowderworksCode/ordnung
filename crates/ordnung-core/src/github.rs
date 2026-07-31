use serde::{Deserialize, Serialize};

use crate::config::GithubSettings;
pub use entl_github::{
    DependabotAutomergeWorkflowFacts, GithubActionPublicationFacts, GithubActionsPermissionsFacts,
    GithubBranchFacts, GithubBranchProtectionFacts, GithubDefaultWorkflowPermissions,
    GithubLicenseFacts, GithubPullRequestAgeFacts, GithubRepositoryFacts, GithubRulesetBypassActor,
    GithubRulesetFacts, GithubSecurityFacts, GithubStaleFacts, GithubValue, GithubWorkflowFacts,
    GithubWorkflowRun,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GithubSettingChange {
    pub setting: GithubSetting,
    pub current: bool,
    pub desired: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GithubSetting {
    AllowAutoMerge,
    DeleteBranchOnMerge,
    AllowUpdateBranch,
}

impl GithubSetting {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AllowAutoMerge => "allow_auto_merge",
            Self::DeleteBranchOnMerge => "delete_branch_on_merge",
            Self::AllowUpdateBranch => "allow_update_branch",
        }
    }
}

pub fn plan_github_settings(
    facts: &GithubRepositoryFacts,
    desired: &GithubSettings,
) -> Vec<GithubSettingChange> {
    let candidates = [
        (
            GithubSetting::AllowAutoMerge,
            facts.allow_auto_merge,
            desired.allow_auto_merge,
        ),
        (
            GithubSetting::DeleteBranchOnMerge,
            facts.delete_branch_on_merge,
            desired.delete_branch_on_merge,
        ),
        (
            GithubSetting::AllowUpdateBranch,
            facts.allow_update_branch,
            desired.allow_update_branch,
        ),
    ];
    candidates
        .into_iter()
        .filter_map(|(setting, current, desired)| {
            desired
                .filter(|desired| *desired != current)
                .map(|desired| GithubSettingChange {
                    setting,
                    current,
                    desired,
                })
        })
        .collect()
}
