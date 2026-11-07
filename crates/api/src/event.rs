use crate::{
    Account, Group, GroupLaunchProgress, ImportProgress, OnlineStats, QueueEntry, UpdateInfo,
    UpdateProgress,
};

#[derive(Debug, Clone, PartialEq)]
pub enum BackendEvent {
    Accounts(Vec<Account>),
    Groups(Vec<Group>),
    ProfilesChanged,
    LaunchQueue(Vec<QueueEntry>),
    GroupLaunch(GroupLaunchProgress),
    Import(ImportProgress),
    UpdateAvailable(UpdateInfo),
    Update(UpdateProgress),
    OnlineStats(OnlineStats),
    LogsChanged,
    FocusRequested,
    QuitRequested,
}
