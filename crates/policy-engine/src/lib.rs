use lantern_protocol::Capability;
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceAccess {
    repository: PathBuf,
    capabilities: BTreeSet<Capability>,
}

#[derive(Clone, Debug, Default)]
pub struct WorkspacePolicy {
    access: Option<WorkspaceAccess>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolicyError {
    WorkspaceNotConfigured,
    RepositoryChanged { expected: PathBuf, actual: PathBuf },
    UnsupportedCapability(Capability),
    MissingCapability(Capability),
    NetworkRequiresRead,
}

impl WorkspacePolicy {
    pub fn configure(
        &mut self,
        repository: PathBuf,
        requested: impl IntoIterator<Item = Capability>,
    ) -> Result<&WorkspaceAccess, PolicyError> {
        if let Some(existing) = &self.access
            && existing.repository != repository
        {
            return Err(PolicyError::RepositoryChanged {
                expected: existing.repository.clone(),
                actual: repository,
            });
        }

        let capabilities = requested.into_iter().collect::<BTreeSet<_>>();
        for capability in &capabilities {
            if matches!(
                capability,
                Capability::RepositoryWrite | Capability::ProcessExecution
            ) {
                return Err(PolicyError::UnsupportedCapability(*capability));
            }
        }
        if capabilities.contains(&Capability::NetworkAccess)
            && !capabilities.contains(&Capability::RepositoryRead)
        {
            return Err(PolicyError::NetworkRequiresRead);
        }

        let access = self.access.insert(WorkspaceAccess {
            repository,
            capabilities,
        });
        Ok(access)
    }

    pub fn authorize(
        &self,
        repository: &Path,
        required: impl IntoIterator<Item = Capability>,
    ) -> Result<(), PolicyError> {
        let access = self
            .access
            .as_ref()
            .ok_or(PolicyError::WorkspaceNotConfigured)?;
        if access.repository != repository {
            return Err(PolicyError::RepositoryChanged {
                expected: access.repository.clone(),
                actual: repository.to_owned(),
            });
        }
        for capability in required {
            if !access.capabilities.contains(&capability) {
                return Err(PolicyError::MissingCapability(capability));
            }
        }
        Ok(())
    }
}

impl WorkspaceAccess {
    pub fn repository(&self) -> &Path {
        &self.repository
    }

    pub fn capabilities(&self) -> Vec<Capability> {
        self.capabilities.iter().copied().collect()
    }
}

impl fmt::Display for PolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkspaceNotConfigured => {
                formatter.write_str("workspace trust is not configured")
            }
            Self::RepositoryChanged { expected, actual } => write!(
                formatter,
                "workspace is bound to {}; refusing repository {}",
                expected.display(),
                actual.display()
            ),
            Self::UnsupportedCapability(capability) => write!(
                formatter,
                "{} is unavailable in read-only Quick Ask",
                capability.label()
            ),
            Self::MissingCapability(capability) => {
                write!(formatter, "{} is not granted", capability.label())
            }
            Self::NetworkRequiresRead => {
                formatter.write_str("model transmission requires repository read access")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from("/workspace/project")
    }

    #[test]
    fn starts_unconfigured_and_denies_reads() {
        let policy = WorkspacePolicy::default();
        assert_eq!(
            policy.authorize(&root(), [Capability::RepositoryRead]),
            Err(PolicyError::WorkspaceNotConfigured)
        );
    }

    #[test]
    fn hard_denials_are_never_stored_as_pending_permissions() {
        let mut policy = WorkspacePolicy::default();
        assert_eq!(
            policy.configure(root(), [Capability::RepositoryWrite]),
            Err(PolicyError::UnsupportedCapability(
                Capability::RepositoryWrite
            ))
        );
        assert_eq!(
            policy.authorize(&root(), [Capability::RepositoryRead]),
            Err(PolicyError::WorkspaceNotConfigured)
        );
    }

    #[test]
    fn every_required_capability_must_be_granted() {
        let mut policy = WorkspacePolicy::default();
        policy
            .configure(root(), [Capability::RepositoryRead])
            .expect("configure read access");
        assert_eq!(
            policy.authorize(
                &root(),
                [Capability::RepositoryRead, Capability::NetworkAccess]
            ),
            Err(PolicyError::MissingCapability(Capability::NetworkAccess))
        );
    }

    #[test]
    fn repository_binding_cannot_be_retargeted() {
        let mut policy = WorkspacePolicy::default();
        policy.configure(root(), []).expect("bind workspace");
        assert!(matches!(
            policy.configure(PathBuf::from("/workspace/other"), []),
            Err(PolicyError::RepositoryChanged { .. })
        ));
    }

    #[test]
    fn network_access_cannot_be_detached_from_read_consent() {
        let mut policy = WorkspacePolicy::default();
        assert_eq!(
            policy.configure(root(), [Capability::NetworkAccess]),
            Err(PolicyError::NetworkRequiresRead)
        );
    }
}
