//! Dependency checking for jq and Docker availability.
//!
//! Checks for required external dependencies at startup and warns if missing.
//! Uses trait-based design for testability.

use std::process::Command;

/// Result of dependency checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyStatus {
    /// Whether jq is available (jq --version exits 0).
    pub jq_available: bool,
    /// Whether Docker is running (docker info exits 0).
    pub docker_available: bool,
}

/// Trait for checking command availability.
///
/// Enables mocking in tests to verify both success and failure paths.
pub trait DependencyChecker {
    /// Check if a command is available and working.
    ///
    /// # Arguments
    /// * `cmd` - The command to run
    /// * `args` - Arguments to pass to the command
    ///
    /// # Returns
    /// `true` if the command exits successfully, `false` otherwise.
    fn check_command(&self, cmd: &str, args: &[&str]) -> bool;
}

/// Real implementation using [`std::process::Command`].
#[derive(Debug, Default, Clone, Copy)]
pub struct RealChecker;

impl DependencyChecker for RealChecker {
    fn check_command(&self, cmd: &str, args: &[&str]) -> bool {
        Command::new(cmd)
            .args(args)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

/// Check if jq is available.
pub fn check_jq<C: DependencyChecker>(checker: &C) -> bool {
    checker.check_command("jq", &["--version"])
}

/// Check if Docker daemon is running.
pub fn check_docker<C: DependencyChecker>(checker: &C) -> bool {
    checker.check_command("docker", &["info"])
}

/// Check all dependencies and return status.
///
/// Does not log warnings - caller is responsible for logging based on status.
pub fn check_all<C: DependencyChecker>(checker: &C) -> DependencyStatus {
    DependencyStatus {
        jq_available: check_jq(checker),
        docker_available: check_docker(checker),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic, clippy::expect_used, clippy::unwrap_used)]

    use super::*;

    /// Mock checker that returns configured values.
    struct MockChecker {
        jq_available: bool,
        docker_available: bool,
    }

    impl MockChecker {
        fn new(jq_available: bool, docker_available: bool) -> Self {
            Self {
                jq_available,
                docker_available,
            }
        }
    }

    impl DependencyChecker for MockChecker {
        fn check_command(&self, cmd: &str, _args: &[&str]) -> bool {
            match cmd {
                "jq" => self.jq_available,
                "docker" => self.docker_available,
                _ => false,
            }
        }
    }

    #[test]
    fn test_check_jq_available() {
        let checker = MockChecker::new(true, false);
        assert!(check_jq(&checker));
    }

    #[test]
    fn test_check_jq_unavailable() {
        let checker = MockChecker::new(false, false);
        assert!(!check_jq(&checker));
    }

    #[test]
    fn test_check_docker_running() {
        let checker = MockChecker::new(false, true);
        assert!(check_docker(&checker));
    }

    #[test]
    fn test_check_docker_not_running() {
        let checker = MockChecker::new(false, false);
        assert!(!check_docker(&checker));
    }

    #[test]
    fn test_check_all_both_present() {
        let checker = MockChecker::new(true, true);
        let status = check_all(&checker);
        assert!(status.jq_available);
        assert!(status.docker_available);
    }

    #[test]
    fn test_check_all_jq_missing() {
        let checker = MockChecker::new(false, true);
        let status = check_all(&checker);
        assert!(!status.jq_available);
        assert!(status.docker_available);
    }

    #[test]
    fn test_check_all_docker_missing() {
        let checker = MockChecker::new(true, false);
        let status = check_all(&checker);
        assert!(status.jq_available);
        assert!(!status.docker_available);
    }

    #[test]
    fn test_check_all_both_missing() {
        let checker = MockChecker::new(false, false);
        let status = check_all(&checker);
        assert!(!status.jq_available);
        assert!(!status.docker_available);
    }

    #[test]
    fn test_real_checker_jq() {
        // Integration test - depends on jq being installed
        let checker = RealChecker;
        // jq should be available on dev machines
        let result = check_jq(&checker);
        // We just verify it doesn't panic - actual availability depends on environment
        let _ = result;
    }

    #[test]
    fn test_real_checker_docker() {
        // Integration test - depends on Docker running
        let checker = RealChecker;
        // Docker may or may not be running
        let result = check_docker(&checker);
        // We just verify it doesn't panic - actual availability depends on environment
        let _ = result;
    }

    #[test]
    fn test_dependency_status_equality() {
        let status1 = DependencyStatus {
            jq_available: true,
            docker_available: false,
        };
        let status2 = DependencyStatus {
            jq_available: true,
            docker_available: false,
        };
        let status3 = DependencyStatus {
            jq_available: false,
            docker_available: false,
        };
        assert_eq!(status1, status2);
        assert_ne!(status1, status3);
    }

    #[test]
    fn test_dependency_status_clone() {
        let status = DependencyStatus {
            jq_available: true,
            docker_available: true,
        };
        let cloned = status.clone();
        assert_eq!(status, cloned);
    }
}
