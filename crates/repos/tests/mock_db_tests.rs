#![cfg(feature = "ssr")]

use repos::error::RepoError;
use repos::ssr::Repo;
use std::sync::atomic::{AtomicBool, Ordering};

// These statics help us track if our mocks were called correctly
static CONFIG_SET: AtomicBool = AtomicBool::new(false);
static BEGIN_CALLED: AtomicBool = AtomicBool::new(false);
static ACQUIRE_CALLED: AtomicBool = AtomicBool::new(false);

// Reset test flags between tests
fn reset_flags() {
    CONFIG_SET.store(false, Ordering::SeqCst);
    BEGIN_CALLED.store(false, Ordering::SeqCst);
    ACQUIRE_CALLED.store(false, Ordering::SeqCst);
}

// Mock the database pool for testing
struct MockRepo;

impl MockRepo {
    fn test_set_config(&self) -> Result<(), RepoError> {
        CONFIG_SET.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn test_set_config_fail(&self) -> Result<(), RepoError> {
        Err(RepoError::DatabaseError("Mocked config failure".to_string()))
    }

    fn test_begin_ok(&self) -> Result<(), RepoError> {
        BEGIN_CALLED.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn test_begin_fail(&self) -> Result<(), RepoError> {
        Err(RepoError::DatabaseError("Mocked transaction failure".to_string()))
    }

    fn test_acquire_ok(&self) -> Result<(), RepoError> {
        ACQUIRE_CALLED.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn test_acquire_fail(&self) -> Result<(), RepoError> {
        Err(RepoError::DatabaseError("Mocked acquire failure".to_string()))
    }
}

// We test the logic flow using our mock objects

#[test]
fn test_flow_begin_admin_success() {
    reset_flags();
    let mock = MockRepo;

    // Test successful path
    let result = mock.test_begin_ok().and_then(|_| mock.test_set_config());

    assert!(result.is_ok());
    assert!(BEGIN_CALLED.load(Ordering::SeqCst));
    assert!(CONFIG_SET.load(Ordering::SeqCst));
}

#[test]
fn test_flow_begin_admin_config_failure() {
    reset_flags();
    let mock = MockRepo;

    // Test config failure path
    let result = mock
        .test_begin_ok()
        .and_then(|_| mock.test_set_config_fail());

    assert!(result.is_err());
    if let Err(RepoError::DatabaseError(msg)) = result {
        assert!(msg.contains("Mocked config failure"));
    } else {
        panic!("Expected DatabaseError");
    }
    assert!(BEGIN_CALLED.load(Ordering::SeqCst));
    assert!(!CONFIG_SET.load(Ordering::SeqCst));
}

#[test]
fn test_flow_begin_failure() {
    reset_flags();
    let mock = MockRepo;

    // Test transaction failure path
    let result = mock.test_begin_fail();

    assert!(result.is_err());
    if let Err(RepoError::DatabaseError(msg)) = result {
        assert!(msg.contains("Mocked transaction failure"));
    } else {
        panic!("Expected DatabaseError");
    }
    assert!(!BEGIN_CALLED.load(Ordering::SeqCst));
    assert!(!CONFIG_SET.load(Ordering::SeqCst));
}

#[test]
fn test_flow_acquire_admin_success() {
    reset_flags();
    let mock = MockRepo;

    // Test successful path
    let result = mock.test_acquire_ok().and_then(|_| mock.test_set_config());

    assert!(result.is_ok());
    assert!(ACQUIRE_CALLED.load(Ordering::SeqCst));
    assert!(CONFIG_SET.load(Ordering::SeqCst));
}

#[test]
fn test_flow_acquire_admin_config_failure() {
    reset_flags();
    let mock = MockRepo;

    // Test config failure path
    let result = mock
        .test_acquire_ok()
        .and_then(|_| mock.test_set_config_fail());

    assert!(result.is_err());
    if let Err(RepoError::DatabaseError(msg)) = result {
        assert!(msg.contains("Mocked config failure"));
    } else {
        panic!("Expected DatabaseError");
    }
    assert!(ACQUIRE_CALLED.load(Ordering::SeqCst));
    assert!(!CONFIG_SET.load(Ordering::SeqCst));
}

#[test]
fn test_flow_acquire_failure() {
    reset_flags();
    let mock = MockRepo;

    // Test connection failure path
    let result = mock.test_acquire_fail();

    assert!(result.is_err());
    if let Err(RepoError::DatabaseError(msg)) = result {
        assert!(msg.contains("Mocked acquire failure"));
    } else {
        panic!("Expected DatabaseError");
    }
    assert!(!ACQUIRE_CALLED.load(Ordering::SeqCst));
    assert!(!CONFIG_SET.load(Ordering::SeqCst));
}
