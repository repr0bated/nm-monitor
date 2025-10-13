// State manager orchestrator - coordinates plugins and provides atomic operations
use crate::ledger::Ledger;
use crate::state::plugin::{ApplyResult, Checkpoint, StateDiff, StatePlugin};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

/// Desired state loaded from YAML/JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesiredState {
    pub version: u32,
    pub plugins: HashMap<String, Value>,
}

/// Current state snapshot across all plugins
#[derive(Debug, Serialize, Deserialize)]
pub struct CurrentState {
    pub plugins: HashMap<String, Value>,
}

/// Report of apply operation
#[derive(Debug, Serialize, Deserialize)]
pub struct ApplyReport {
    pub success: bool,
    pub results: Vec<ApplyResult>,
    pub checkpoints: Vec<(String, Checkpoint)>,
}

/// State manager coordinates all plugins and provides atomic operations
pub struct StateManager {
    plugins: Arc<RwLock<HashMap<String, Box<dyn StatePlugin>>>>,
    ledger: Arc<Mutex<Ledger>>,
}

impl StateManager {
    /// Create a new state manager
    pub fn new(ledger: Arc<Mutex<Ledger>>) -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            ledger,
        }
    }

    /// Register a state plugin
    pub async fn register_plugin(&self, plugin: Box<dyn StatePlugin>) {
        let name = plugin.name().to_string();
        let mut plugins = self.plugins.write().await;
        plugins.insert(name.clone(), plugin);
        log::info!("Registered state plugin: {}", name);
    }

    /// Load desired state from YAML/JSON file
    pub async fn load_desired_state(&self, path: &Path) -> Result<DesiredState> {
        let content = tokio::fs::read_to_string(path).await?;

        // Try YAML first, fall back to JSON
        if path.extension().and_then(|s| s.to_str()) == Some("yaml")
            || path.extension().and_then(|s| s.to_str()) == Some("yml")
        {
            Ok(serde_yaml::from_str(&content)?)
        } else {
            Ok(serde_json::from_str(&content)?)
        }
    }

    /// Query current state across all plugins
    pub async fn query_current_state(&self) -> Result<CurrentState> {
        let plugins = self.plugins.read().await;
        let mut state = HashMap::new();

        for (name, plugin) in plugins.iter() {
            match plugin.query_current_state().await {
                Ok(plugin_state) => {
                    state.insert(name.clone(), plugin_state);
                }
                Err(e) => {
                    log::error!("Failed to query plugin {}: {}", name, e);
                    return Err(anyhow!("Failed to query plugin {}: {}", name, e));
                }
            }
        }

        Ok(CurrentState { plugins: state })
    }

    /// Query state from a specific plugin
    pub async fn query_plugin_state(&self, plugin_name: &str) -> Result<Value> {
        let plugins = self.plugins.read().await;

        match plugins.get(plugin_name) {
            Some(plugin) => plugin.query_current_state().await,
            None => Err(anyhow!("Plugin not found: {}", plugin_name)),
        }
    }

    /// Calculate diffs for all plugins
    async fn calculate_all_diffs(&self, desired: &DesiredState) -> Result<Vec<StateDiff>> {
        let plugins = self.plugins.read().await;
        let mut diffs = Vec::new();

        for (plugin_name, desired_state) in &desired.plugins {
            if let Some(plugin) = plugins.get(plugin_name) {
                let current_state = plugin.query_current_state().await?;
                let diff = plugin.calculate_diff(&current_state, desired_state).await?;

                // Only include diffs that have actual actions
                if !diff.actions.is_empty() {
                    diffs.push(diff);
                }
            } else {
                log::warn!("Plugin {} not registered, skipping", plugin_name);
            }
        }

        Ok(diffs)
    }

    /// Verify all states match desired
    async fn verify_all_states(&self, desired: &DesiredState) -> Result<bool> {
        let plugins = self.plugins.read().await;

        for (plugin_name, desired_state) in &desired.plugins {
            if let Some(plugin) = plugins.get(plugin_name) {
                if !plugin.verify_state(desired_state).await? {
                    log::error!("State verification failed for plugin: {}", plugin_name);
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Apply desired state atomically across all plugins
    pub async fn apply_state(&self, desired: DesiredState) -> Result<ApplyReport> {
        let plugins = self.plugins.read().await;
        let mut checkpoints = Vec::new();
        let mut results = Vec::new();

        log::info!("Starting atomic state apply operation");

        // Phase 1: Create checkpoints for all affected plugins
        log::info!("Phase 1: Creating checkpoints");
        for (plugin_name, _desired_state) in desired.plugins.iter() {
            if let Some(plugin) = plugins.get(plugin_name) {
                match plugin.create_checkpoint().await {
                    Ok(checkpoint) => {
                        log::info!("Created checkpoint for plugin: {}", plugin_name);
                        checkpoints.push((plugin_name.clone(), checkpoint));
                    }
                    Err(e) => {
                        log::error!("Failed to create checkpoint for {}: {}", plugin_name, e);
                        // Continue without checkpoint if plugin doesn't support it
                    }
                }
            }
        }

        // Phase 2: Calculate diffs
        log::info!("Phase 2: Calculating diffs");
        let diffs = match self.calculate_all_diffs(&desired).await {
            Ok(diffs) => diffs,
            Err(e) => {
                log::error!("Failed to calculate diffs: {}", e);
                return Err(e);
            }
        };

        if diffs.is_empty() {
            log::info!("No changes needed - current state matches desired state");
            return Ok(ApplyReport {
                success: true,
                results,
                checkpoints,
            });
        }

        // Phase 3: Apply changes in dependency order
        log::info!("Phase 3: Applying changes ({} plugins)", diffs.len());
        for diff in diffs {
            let plugin = plugins.get(&diff.plugin).unwrap();

            match plugin.apply_state(&diff).await {
                Ok(result) => {
                    log::info!("Applied state for plugin: {}", diff.plugin);

                    // Log to blockchain ledger
                    if let Ok(mut ledger) = self.ledger.try_lock() {
                        if let Err(e) = ledger.append(
                            "apply_state",
                            serde_json::json!({
                                "plugin": diff.plugin,
                                "result": result,
                            }),
                        ) {
                            log::error!("Failed to log to ledger: {}", e);
                        }
                    }

                    results.push(result);
                }
                Err(e) => {
                    log::error!(
                        "State apply failed for {}: {}, rolling back",
                        diff.plugin,
                        e
                    );
                    self.rollback_all(&checkpoints).await?;
                    return Err(e);
                }
            }
        }

        // Phase 4: Verify all states match desired
        log::info!("Phase 4: Verifying state");
        let verified = self.verify_all_states(&desired).await?;

        if !verified {
            log::error!("State verification failed, rolling back");
            self.rollback_all(&checkpoints).await?;
            return Err(anyhow!("State verification failed"));
        }

        log::info!("State apply completed successfully");
        Ok(ApplyReport {
            success: true,
            results,
            checkpoints,
        })
    }

    /// Rollback all plugins to checkpoints
    async fn rollback_all(&self, checkpoints: &[(String, Checkpoint)]) -> Result<()> {
        let plugins = self.plugins.read().await;

        log::warn!("Rolling back {} plugins", checkpoints.len());

        // Rollback in reverse order
        for (plugin_name, checkpoint) in checkpoints.iter().rev() {
            if let Some(plugin) = plugins.get(plugin_name) {
                if let Err(e) = plugin.rollback(checkpoint).await {
                    log::error!("Failed to rollback plugin {}: {}", plugin_name, e);
                    // Continue rolling back other plugins
                }

                // Log rollback to blockchain
                if let Ok(mut ledger) = self.ledger.try_lock() {
                    if let Err(e) = ledger.append(
                        "rollback",
                        serde_json::json!({
                            "plugin": plugin_name,
                            "checkpoint_id": checkpoint.id
                        }),
                    ) {
                        log::error!("Failed to log rollback to ledger: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Show diff between current and desired state
    pub async fn show_diff(&self, desired: DesiredState) -> Result<Vec<StateDiff>> {
        self.calculate_all_diffs(&desired).await
    }
}
