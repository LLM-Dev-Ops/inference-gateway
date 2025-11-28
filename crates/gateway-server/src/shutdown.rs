//! Graceful shutdown handling for the gateway.
//!
//! Provides enterprise-grade shutdown coordination including:
//! - Configurable shutdown timeout and drain period
//! - In-flight request tracking and completion
//! - Connection draining with deadlines
//! - Background task cancellation
//! - Shutdown state broadcasting
//! - Health endpoint coordination

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::{broadcast, watch, Notify};
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};

/// Graceful shutdown configuration
#[derive(Debug, Clone)]
pub struct ShutdownConfig {
    /// Maximum time to wait for in-flight requests to complete
    pub graceful_timeout: Duration,
    /// Time to wait before forcefully closing connections
    pub drain_timeout: Duration,
    /// Interval to log shutdown progress
    pub progress_interval: Duration,
    /// Whether to reject new requests during shutdown
    pub reject_new_requests: bool,
    /// Time to wait after signaling shutdown before starting drain
    pub pre_drain_delay: Duration,
}

impl Default for ShutdownConfig {
    fn default() -> Self {
        Self {
            graceful_timeout: Duration::from_secs(30),
            drain_timeout: Duration::from_secs(5),
            progress_interval: Duration::from_secs(1),
            reject_new_requests: true,
            pre_drain_delay: Duration::from_millis(500),
        }
    }
}

impl ShutdownConfig {
    /// Create a new shutdown configuration
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the graceful timeout
    #[must_use]
    pub fn with_graceful_timeout(mut self, timeout: Duration) -> Self {
        self.graceful_timeout = timeout;
        self
    }

    /// Set the drain timeout
    #[must_use]
    pub fn with_drain_timeout(mut self, timeout: Duration) -> Self {
        self.drain_timeout = timeout;
        self
    }

    /// Set whether to reject new requests during shutdown
    #[must_use]
    pub fn with_reject_new_requests(mut self, reject: bool) -> Self {
        self.reject_new_requests = reject;
        self
    }
}

/// Shutdown phase
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownPhase {
    /// Normal operation
    Running,
    /// Shutdown initiated, draining connections
    Draining,
    /// Force closing remaining connections
    ForceClose,
    /// Shutdown complete
    Complete,
}

impl std::fmt::Display for ShutdownPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "running"),
            Self::Draining => write!(f, "draining"),
            Self::ForceClose => write!(f, "force_close"),
            Self::Complete => write!(f, "complete"),
        }
    }
}

/// Shutdown coordinator for managing graceful shutdown
pub struct ShutdownCoordinator {
    config: ShutdownConfig,
    /// Current shutdown phase
    phase: watch::Sender<ShutdownPhase>,
    /// Phase receiver for subscribers
    phase_rx: watch::Receiver<ShutdownPhase>,
    /// Notification for shutdown trigger
    shutdown_notify: Arc<Notify>,
    /// Whether shutdown has been triggered
    shutdown_triggered: AtomicBool,
    /// Counter for in-flight requests
    in_flight_requests: AtomicU64,
    /// Broadcast channel for shutdown events
    shutdown_tx: broadcast::Sender<ShutdownEvent>,
    /// List of registered background tasks
    background_tasks: Arc<tokio::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

/// Shutdown event
#[derive(Debug, Clone)]
pub enum ShutdownEvent {
    /// Shutdown initiated
    Initiated {
        /// The reason for shutdown
        reason: String,
    },
    /// Phase changed
    PhaseChanged {
        /// The new shutdown phase
        phase: ShutdownPhase,
    },
    /// Request drain progress
    DrainProgress {
        /// Number of remaining in-flight requests
        remaining: u64,
    },
    /// Shutdown complete
    Complete,
}

impl ShutdownCoordinator {
    /// Create a new shutdown coordinator
    #[must_use]
    pub fn new(config: ShutdownConfig) -> Self {
        let (phase_tx, phase_rx) = watch::channel(ShutdownPhase::Running);
        let (shutdown_tx, _) = broadcast::channel(16);

        Self {
            config,
            phase: phase_tx,
            phase_rx,
            shutdown_notify: Arc::new(Notify::new()),
            shutdown_triggered: AtomicBool::new(false),
            in_flight_requests: AtomicU64::new(0),
            shutdown_tx,
            background_tasks: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }

    /// Create with default configuration
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(ShutdownConfig::default())
    }

    /// Check if shutdown is in progress
    #[must_use]
    pub fn is_shutting_down(&self) -> bool {
        self.shutdown_triggered.load(Ordering::SeqCst)
    }

    /// Get current shutdown phase
    #[must_use]
    pub fn current_phase(&self) -> ShutdownPhase {
        *self.phase_rx.borrow()
    }

    /// Get the number of in-flight requests
    #[must_use]
    pub fn in_flight_count(&self) -> u64 {
        self.in_flight_requests.load(Ordering::SeqCst)
    }

    /// Subscribe to phase changes
    #[must_use]
    pub fn subscribe_phase(&self) -> watch::Receiver<ShutdownPhase> {
        self.phase_rx.clone()
    }

    /// Subscribe to shutdown events
    #[must_use]
    pub fn subscribe_events(&self) -> broadcast::Receiver<ShutdownEvent> {
        self.shutdown_tx.subscribe()
    }

    /// Get a shutdown signal future
    pub fn shutdown_signal(&self) -> impl std::future::Future<Output = ()> + Send + 'static {
        let notify = self.shutdown_notify.clone();
        async move {
            notify.notified().await;
        }
    }

    /// Register a request start
    pub fn request_start(&self) {
        self.in_flight_requests.fetch_add(1, Ordering::SeqCst);
    }

    /// Register a request completion
    pub fn request_complete(&self) {
        let prev = self.in_flight_requests.fetch_sub(1, Ordering::SeqCst);
        debug!(in_flight = prev - 1, "Request completed");
    }

    /// Register a background task
    pub async fn register_task(&self, handle: tokio::task::JoinHandle<()>) {
        let mut tasks = self.background_tasks.lock().await;
        tasks.push(handle);
    }

    /// Should accept new requests?
    #[must_use]
    pub fn should_accept_requests(&self) -> bool {
        if !self.config.reject_new_requests {
            return true;
        }
        !self.is_shutting_down()
    }

    /// Trigger shutdown with a reason
    pub async fn trigger_shutdown(&self, reason: &str) {
        if self
            .shutdown_triggered
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            info!("Shutdown already triggered");
            return;
        }

        info!(reason = %reason, "Initiating graceful shutdown");

        // Broadcast shutdown initiated
        let _ = self.shutdown_tx.send(ShutdownEvent::Initiated {
            reason: reason.to_string(),
        });

        // Pre-drain delay
        if !self.config.pre_drain_delay.is_zero() {
            debug!(
                delay_ms = self.config.pre_drain_delay.as_millis(),
                "Pre-drain delay"
            );
            sleep(self.config.pre_drain_delay).await;
        }

        // Enter draining phase
        self.set_phase(ShutdownPhase::Draining).await;

        // Wait for in-flight requests with timeout
        let drain_result =
            timeout(self.config.graceful_timeout, self.wait_for_drain()).await;

        match drain_result {
            Ok(()) => {
                info!("All in-flight requests completed");
            }
            Err(_) => {
                let remaining = self.in_flight_count();
                warn!(
                    remaining = remaining,
                    "Graceful timeout exceeded, forcing shutdown"
                );
                self.set_phase(ShutdownPhase::ForceClose).await;

                // Additional drain timeout
                sleep(self.config.drain_timeout).await;
            }
        }

        // Cancel background tasks
        self.cancel_background_tasks().await;

        // Complete
        self.set_phase(ShutdownPhase::Complete).await;
        let _ = self.shutdown_tx.send(ShutdownEvent::Complete);

        // Notify waiters
        self.shutdown_notify.notify_waiters();

        info!("Graceful shutdown complete");
    }

    /// Set the shutdown phase
    async fn set_phase(&self, phase: ShutdownPhase) {
        info!(phase = %phase, "Shutdown phase changed");
        let _ = self.phase.send(phase);
        let _ = self
            .shutdown_tx
            .send(ShutdownEvent::PhaseChanged { phase });
    }

    /// Wait for all in-flight requests to complete
    async fn wait_for_drain(&self) {
        let mut last_logged = std::time::Instant::now();

        loop {
            let count = self.in_flight_count();
            if count == 0 {
                break;
            }

            // Log progress periodically
            if last_logged.elapsed() >= self.config.progress_interval {
                info!(remaining = count, "Waiting for in-flight requests");
                let _ = self
                    .shutdown_tx
                    .send(ShutdownEvent::DrainProgress { remaining: count });
                last_logged = std::time::Instant::now();
            }

            sleep(Duration::from_millis(50)).await;
        }
    }

    /// Cancel all registered background tasks
    async fn cancel_background_tasks(&self) {
        let mut tasks = self.background_tasks.lock().await;
        let task_count = tasks.len();

        if task_count > 0 {
            info!(count = task_count, "Cancelling background tasks");

            for handle in tasks.drain(..) {
                handle.abort();
            }
        }
    }
}

/// Request guard that tracks request lifecycle
pub struct RequestGuard {
    coordinator: Arc<ShutdownCoordinator>,
}

impl RequestGuard {
    /// Create a new request guard
    #[must_use]
    pub fn new(coordinator: Arc<ShutdownCoordinator>) -> Option<Self> {
        if !coordinator.should_accept_requests() {
            return None;
        }
        coordinator.request_start();
        Some(Self { coordinator })
    }
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        self.coordinator.request_complete();
    }
}

/// Shutdown signal handler with multiple signal support
///
/// # Panics
/// Panics if signal handlers cannot be installed
#[allow(clippy::expect_used)]
pub async fn shutdown_signal() -> String {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
        "ctrl+c"
    };

    #[cfg(unix)]
    let sigterm = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
        "sigterm"
    };

    #[cfg(unix)]
    let sigint = async {
        signal::unix::signal(signal::unix::SignalKind::interrupt())
            .expect("failed to install SIGINT handler")
            .recv()
            .await;
        "sigint"
    };

    #[cfg(unix)]
    let sigquit = async {
        signal::unix::signal(signal::unix::SignalKind::quit())
            .expect("failed to install SIGQUIT handler")
            .recv()
            .await;
        "sigquit"
    };

    #[cfg(not(unix))]
    let sigterm = std::future::pending::<&str>();
    #[cfg(not(unix))]
    let sigint = std::future::pending::<&str>();
    #[cfg(not(unix))]
    let sigquit = std::future::pending::<&str>();

    let signal_name = tokio::select! {
        name = ctrl_c => name,
        name = sigterm => name,
        name = sigint => name,
        name = sigquit => name,
    };

    info!(signal = signal_name, "Received shutdown signal");
    signal_name.to_string()
}

/// Enhanced server runner with shutdown coordination
pub struct GracefulServer {
    shutdown_coordinator: Arc<ShutdownCoordinator>,
}

impl GracefulServer {
    /// Create a new graceful server wrapper
    #[must_use]
    pub fn new(config: ShutdownConfig) -> Self {
        Self {
            shutdown_coordinator: Arc::new(ShutdownCoordinator::new(config)),
        }
    }

    /// Get the shutdown coordinator
    #[must_use]
    pub fn coordinator(&self) -> Arc<ShutdownCoordinator> {
        self.shutdown_coordinator.clone()
    }

    /// Run until shutdown signal received
    pub async fn run_until_shutdown<F, Fut>(&self, server_fn: F)
    where
        F: FnOnce(Arc<ShutdownCoordinator>) -> Fut,
        Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    {
        let coordinator = self.shutdown_coordinator.clone();
        let shutdown_handle = coordinator.clone();

        // Spawn signal listener
        let signal_task = tokio::spawn(async move {
            let reason = shutdown_signal().await;
            shutdown_handle.trigger_shutdown(&reason).await;
        });

        // Run server
        match server_fn(coordinator.clone()).await {
            Ok(()) => {
                info!("Server stopped normally");
            }
            Err(e) => {
                error!(error = %e, "Server error");
                coordinator.trigger_shutdown("server error").await;
            }
        }

        // Ensure signal task is cleaned up
        signal_task.abort();
    }
}

/// Shutdown statistics
#[derive(Debug, Clone, Default)]
pub struct ShutdownStats {
    /// Time shutdown was initiated
    pub initiated_at: Option<std::time::Instant>,
    /// Time shutdown completed
    pub completed_at: Option<std::time::Instant>,
    /// Number of requests drained
    pub requests_drained: u64,
    /// Number of requests force-closed
    pub requests_force_closed: u64,
    /// Number of background tasks cancelled
    pub tasks_cancelled: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shutdown_config_defaults() {
        let config = ShutdownConfig::default();
        assert_eq!(config.graceful_timeout, Duration::from_secs(30));
        assert_eq!(config.drain_timeout, Duration::from_secs(5));
        assert!(config.reject_new_requests);
    }

    #[tokio::test]
    async fn test_shutdown_coordinator_initial_state() {
        let coordinator = ShutdownCoordinator::with_defaults();
        assert!(!coordinator.is_shutting_down());
        assert_eq!(coordinator.current_phase(), ShutdownPhase::Running);
        assert_eq!(coordinator.in_flight_count(), 0);
    }

    #[tokio::test]
    async fn test_request_tracking() {
        let coordinator = ShutdownCoordinator::with_defaults();

        coordinator.request_start();
        assert_eq!(coordinator.in_flight_count(), 1);

        coordinator.request_start();
        assert_eq!(coordinator.in_flight_count(), 2);

        coordinator.request_complete();
        assert_eq!(coordinator.in_flight_count(), 1);

        coordinator.request_complete();
        assert_eq!(coordinator.in_flight_count(), 0);
    }

    #[tokio::test]
    async fn test_request_guard() {
        let coordinator = Arc::new(ShutdownCoordinator::with_defaults());

        {
            let _guard = RequestGuard::new(coordinator.clone());
            assert_eq!(coordinator.in_flight_count(), 1);
        }

        // Guard dropped
        assert_eq!(coordinator.in_flight_count(), 0);
    }

    #[tokio::test]
    async fn test_shutdown_trigger() {
        let coordinator = Arc::new(ShutdownCoordinator::new(ShutdownConfig {
            graceful_timeout: Duration::from_millis(100),
            drain_timeout: Duration::from_millis(50),
            progress_interval: Duration::from_millis(10),
            pre_drain_delay: Duration::from_millis(0),
            reject_new_requests: true,
        }));

        // Subscribe to events
        let mut events = coordinator.subscribe_events();

        // Trigger shutdown
        let coord = coordinator.clone();
        tokio::spawn(async move {
            coord.trigger_shutdown("test").await;
        });

        // Should receive initiated event
        let event = events.recv().await.unwrap();
        assert!(matches!(event, ShutdownEvent::Initiated { .. }));

        // Should eventually complete
        loop {
            if let Ok(event) = events.recv().await {
                if matches!(event, ShutdownEvent::Complete) {
                    break;
                }
            }
        }

        assert!(coordinator.is_shutting_down());
        assert_eq!(coordinator.current_phase(), ShutdownPhase::Complete);
    }

    #[tokio::test]
    async fn test_reject_requests_during_shutdown() {
        let coordinator = Arc::new(ShutdownCoordinator::new(ShutdownConfig {
            graceful_timeout: Duration::from_millis(100),
            pre_drain_delay: Duration::from_millis(0),
            reject_new_requests: true,
            ..Default::default()
        }));

        assert!(coordinator.should_accept_requests());

        // Start shutdown
        coordinator
            .shutdown_triggered
            .store(true, Ordering::SeqCst);

        assert!(!coordinator.should_accept_requests());

        // Request guard should return None
        let guard = RequestGuard::new(coordinator.clone());
        assert!(guard.is_none());
    }

    #[tokio::test]
    async fn test_phase_subscription() {
        let coordinator = Arc::new(ShutdownCoordinator::new(ShutdownConfig {
            graceful_timeout: Duration::from_millis(50),
            drain_timeout: Duration::from_millis(10),
            pre_drain_delay: Duration::from_millis(0),
            ..Default::default()
        }));

        let mut phase_rx = coordinator.subscribe_phase();
        assert_eq!(*phase_rx.borrow(), ShutdownPhase::Running);

        let coord = coordinator.clone();
        tokio::spawn(async move {
            coord.trigger_shutdown("test").await;
        });

        // Wait for phase to change to draining
        phase_rx.changed().await.unwrap();
        let phase = *phase_rx.borrow();
        assert!(
            phase == ShutdownPhase::Draining || phase == ShutdownPhase::Complete,
            "Expected Draining or Complete, got {:?}",
            phase
        );
    }

    #[tokio::test]
    async fn test_graceful_server() {
        let server = GracefulServer::new(ShutdownConfig {
            graceful_timeout: Duration::from_millis(100),
            pre_drain_delay: Duration::from_millis(0),
            ..Default::default()
        });

        let coordinator = server.coordinator();
        assert!(!coordinator.is_shutting_down());
    }

    #[test]
    fn test_shutdown_phase_display() {
        assert_eq!(ShutdownPhase::Running.to_string(), "running");
        assert_eq!(ShutdownPhase::Draining.to_string(), "draining");
        assert_eq!(ShutdownPhase::ForceClose.to_string(), "force_close");
        assert_eq!(ShutdownPhase::Complete.to_string(), "complete");
    }

    #[tokio::test]
    async fn test_background_task_registration() {
        let coordinator = ShutdownCoordinator::with_defaults();

        let handle = tokio::spawn(async {
            tokio::time::sleep(Duration::from_secs(100)).await;
        });

        coordinator.register_task(handle).await;

        let tasks = coordinator.background_tasks.lock().await;
        assert_eq!(tasks.len(), 1);
    }

    #[tokio::test]
    async fn test_drain_with_inflight_requests() {
        let coordinator = Arc::new(ShutdownCoordinator::new(ShutdownConfig {
            graceful_timeout: Duration::from_secs(5),
            drain_timeout: Duration::from_millis(50),
            progress_interval: Duration::from_millis(10),
            pre_drain_delay: Duration::from_millis(0),
            reject_new_requests: false, // Allow requests during drain for test
        }));

        // Start some "requests"
        let coord = coordinator.clone();
        let request_task = tokio::spawn(async move {
            coord.request_start();
            coord.request_start();

            // Simulate request processing
            sleep(Duration::from_millis(100)).await;

            coord.request_complete();
            coord.request_complete();
        });

        // Give requests time to start
        sleep(Duration::from_millis(10)).await;

        // Trigger shutdown
        let shutdown_task = {
            let coord = coordinator.clone();
            tokio::spawn(async move {
                coord.trigger_shutdown("test").await;
            })
        };

        // Wait for both
        let _ = tokio::join!(request_task, shutdown_task);

        assert!(coordinator.is_shutting_down());
        assert_eq!(coordinator.in_flight_count(), 0);
    }
}
