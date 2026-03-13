//! Pipeline orchestration — processing pipeline events and executing workflows.
//!
//! This module is the heart of Phase 6. It:
//! 1. Receives `PipelineEvent`s from the knot consumer channel.
//! 2. Parses the pipeline record, validates ownership, and maps workflows to the engine.
//! 3. Builds pipeline environment variables from trigger metadata.
//! 4. Enqueues a job to the bounded queue for each pipeline.
//! 5. Executes all workflows in parallel within each job:
//!    `setup_workflow` → run steps sequentially → `destroy_workflow`.
//! 6. Manages status transitions and log streaming.

use std::io::Write;
use std::sync::Arc;

use spindle_db::Database;
use spindle_engine::traits::{EngineError, PipelineCloneOpts, PipelineWorkflow};
use spindle_engine::{Engine, NixEngine};
use spindle_knot::{PipelineEvent, PipelineRecord};
use spindle_models::{
    FileWorkflowLogger, Pipeline, PipelineEnvVars, PipelineId, StatusKind, TriggerMetadata,
    WorkflowId, WorkflowLogger,
};
use spindle_queue::JobQueue;
use spindle_secrets::Manager as SecretsManager;
use tracing::{error, info, warn};

use crate::notifier::Notifier;

// ---------------------------------------------------------------------------
// KnotSubscriber newtype wrapper
// ---------------------------------------------------------------------------

/// Newtype wrapper around [`KnotConsumer`] that implements [`KnotSubscriber`].
///
/// Required because both `KnotSubscriber` and `KnotConsumer` are defined in
/// external crates (orphan rule).
pub struct KnotSubscriberAdapter(pub Arc<spindle_knot::KnotConsumer>);

#[async_trait::async_trait]
impl spindle_jetstream::ingester::KnotSubscriber for KnotSubscriberAdapter {
    async fn subscribe(&self, knot: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.0.subscribe(knot).await.map_err(|e| Box::new(e) as _)
    }

    async fn unsubscribe(
        &self,
        knot: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.0.unsubscribe(knot).await.map_err(|e| Box::new(e) as _)
    }
}

/// Shared context for the pipeline orchestrator.
#[allow(dead_code)]
pub struct OrchestratorContext {
    /// Database handle.
    pub db: Arc<Database>,
    /// The Nix engine for workflow execution.
    pub engine: Arc<NixEngine>,
    /// Secrets manager.
    pub secrets: Arc<dyn SecretsManager + Send + Sync>,
    /// Job queue.
    pub queue: Arc<JobQueue>,
    /// Event notifier (broadcast channel for WebSocket clients).
    pub notifier: Arc<Notifier>,
    /// This spindle's hostname.
    pub hostname: String,
    /// This spindle's `did:web:{hostname}`.
    pub did_web: String,
    /// Directory for workflow log files.
    pub log_dir: std::path::PathBuf,
    /// Whether dev mode is enabled.
    pub dev: bool,
}

/// Process a pipeline event received from a knot consumer.
///
/// Parses the event, validates it, creates workflows, and submits a job
/// to the queue for execution.
pub fn process_pipeline_event(ctx: Arc<OrchestratorContext>, event: PipelineEvent) {
    let knot = event.knot.clone();
    let rkey = event.rkey.clone().unwrap_or_default();

    // Parse the pipeline record from the event payload.
    let record: PipelineRecord = match serde_json::from_value(event.payload.clone()) {
        Ok(r) => r,
        Err(e) => {
            warn!(%e, knot = %knot, "failed to parse pipeline record from event");
            return;
        }
    };

    let pipeline_id = PipelineId {
        knot: knot.clone(),
        rkey: rkey.clone(),
    };

    // Extract repo info from the record.
    let repo_did = match &record.did {
        Some(d) => d.clone(),
        None => {
            // Fall back to event-level DID.
            match &event.did {
                Some(d) => d.clone(),
                None => {
                    warn!(pipeline = %pipeline_id, "pipeline event missing repo DID");
                    return;
                }
            }
        }
    };

    let repo_name = match &record.repo {
        Some(n) => n.clone(),
        None => {
            warn!(pipeline = %pipeline_id, "pipeline event missing repo name");
            return;
        }
    };

    info!(
        pipeline = %pipeline_id,
        repo_did = %repo_did,
        repo_name = %repo_name,
        "processing pipeline event"
    );

    // Extract workflows from the record.
    let workflow_manifests = match &record.workflows {
        Some(wfs) if !wfs.is_empty() => wfs.clone(),
        _ => {
            warn!(pipeline = %pipeline_id, "pipeline event has no workflows");
            return;
        }
    };

    // Parse trigger metadata.
    let trigger: Option<TriggerMetadata> = record
        .trigger
        .as_ref()
        .and_then(|t| serde_json::from_value(t.clone()).ok());

    // Parse clone options.
    let clone_opts: Option<PipelineCloneOpts> = record
        .clone
        .as_ref()
        .and_then(|c| serde_json::from_value(c.clone()).ok());

    // Build pipeline environment variables.
    let pipeline_env = PipelineEnvVars::build(trigger.as_ref(), &pipeline_id, ctx.dev);

    // Create the Pipeline struct.
    let pipeline = Pipeline {
        repo_owner: repo_did.clone(),
        repo_name: repo_name.clone(),
        workflows: Vec::new(), // Workflows are processed individually below.
    };

    // Initialize workflows via the engine.
    let mut workflow_entries = Vec::new();

    for (filename, manifest) in &workflow_manifests {
        let wf_name = manifest
            .name
            .as_deref()
            .unwrap_or(filename.trim_end_matches(".yml").trim_end_matches(".yaml"));

        let engine_name = manifest.engine.as_deref().unwrap_or("nix");
        let raw_content = match &manifest.content {
            Some(c) => c.clone(),
            None => {
                warn!(
                    pipeline = %pipeline_id,
                    workflow = %wf_name,
                    "workflow manifest has no content, skipping"
                );
                continue;
            }
        };

        let pipeline_workflow = PipelineWorkflow {
            name: wf_name.to_string(),
            engine: engine_name.to_string(),
            raw: raw_content,
            clone: clone_opts.clone(),
        };

        match ctx.engine.init_workflow(pipeline_workflow, &pipeline) {
            Ok(workflow) => {
                let wid = WorkflowId::new(pipeline_id.clone(), wf_name);
                workflow_entries.push((wid, workflow));
            }
            Err(e) => {
                warn!(
                    %e,
                    pipeline = %pipeline_id,
                    workflow = %wf_name,
                    "failed to initialize workflow, skipping"
                );
            }
        }
    }

    if workflow_entries.is_empty() {
        warn!(pipeline = %pipeline_id, "no valid workflows in pipeline");
        return;
    }

    // Register all workflows as pending in the database.
    for (wid, _) in &workflow_entries {
        let wid_str = wid.to_string();
        if let Err(e) =
            ctx.db
                .status_pending(&wid_str, &pipeline_id.knot, &pipeline_id.rkey, &wid.name)
        {
            error!(%e, workflow_id = %wid_str, "failed to create pending status");
        }

        // Emit a status event.
        emit_status_event(&ctx, &wid_str, StatusKind::Pending);
    }

    // Collect workflow IDs before moving entries into the closure.
    let workflow_ids: Vec<WorkflowId> = workflow_entries
        .iter()
        .map(|(wid, _)| wid.clone())
        .collect();

    // Submit a job to execute all workflows.
    let job_ctx = ctx.clone();
    let job_pipeline_id = pipeline_id.clone();
    let job_repo_path = format!("{}/{}", repo_did, repo_name);

    let result = ctx.queue.submit(Box::new(move || {
        Box::pin(async move {
            execute_pipeline(
                job_ctx,
                job_pipeline_id,
                &job_repo_path,
                workflow_entries,
                pipeline_env,
            )
            .await;
        })
    }));

    match result {
        Ok(()) => {
            info!(pipeline = %pipeline_id, "pipeline job submitted to queue");
        }
        Err(e) => {
            error!(%e, pipeline = %pipeline_id, "failed to submit pipeline job to queue");
            // Mark all workflows as failed.
            for wid in &workflow_ids {
                let wid_str = wid.to_string();
                let _ = ctx.db.status_failed(&wid_str);
                emit_status_event(&ctx, &wid_str, StatusKind::Failed);
            }
        }
    }
}

/// Execute all workflows in a pipeline concurrently.
async fn execute_pipeline(
    ctx: Arc<OrchestratorContext>,
    pipeline_id: PipelineId,
    repo_path: &str,
    workflows: Vec<(WorkflowId, spindle_models::Workflow)>,
    pipeline_env: Option<std::collections::HashMap<String, String>>,
) {
    info!(
        pipeline = %pipeline_id,
        workflow_count = workflows.len(),
        "executing pipeline"
    );

    // Fetch secrets for this repo.
    let secrets = match ctx.secrets.get_secrets_unlocked(repo_path).await {
        Ok(s) => s,
        Err(e) => {
            warn!(%e, repo = %repo_path, "failed to fetch secrets, proceeding without");
            Vec::new()
        }
    };

    // Execute all workflows in parallel.
    let mut handles = Vec::new();

    for (wid, mut workflow) in workflows {
        // Merge pipeline env vars into the workflow's environment.
        if let Some(ref env) = pipeline_env {
            for (k, v) in env {
                workflow
                    .environment
                    .entry(k.clone())
                    .or_insert_with(|| v.clone());
            }
        }

        let ctx = ctx.clone();
        let secrets = secrets.clone();

        handles.push(tokio::spawn(async move {
            execute_workflow(ctx, wid, workflow, secrets).await;
        }));
    }

    // Wait for all workflows to complete.
    for handle in handles {
        if let Err(e) = handle.await {
            error!(%e, pipeline = %pipeline_id, "workflow task panicked");
        }
    }

    info!(pipeline = %pipeline_id, "pipeline execution complete");
}

/// Execute a single workflow: setup → run steps → destroy.
async fn execute_workflow(
    ctx: Arc<OrchestratorContext>,
    wid: WorkflowId,
    workflow: spindle_models::Workflow,
    secrets: Vec<spindle_models::UnlockedSecret>,
) {
    let wid_str = wid.to_string();
    let timeout = ctx.engine.workflow_timeout();

    info!(workflow_id = %wid_str, name = %workflow.name, "starting workflow execution");

    // Transition to running.
    if let Err(e) = ctx.db.status_running(&wid_str) {
        error!(%e, workflow_id = %wid_str, "failed to set running status");
    }
    emit_status_event(&ctx, &wid_str, StatusKind::Running);

    // Create the workflow logger.
    let secret_values: Vec<String> = secrets.iter().map(|s| s.value.clone()).collect();
    let logger: Arc<dyn WorkflowLogger> =
        match FileWorkflowLogger::new(&ctx.log_dir, &wid, &secret_values) {
            Ok(l) => Arc::new(l),
            Err(e) => {
                error!(%e, workflow_id = %wid_str, "failed to create workflow logger");
                let _ = ctx.db.status_failed(&wid_str);
                emit_status_event(&ctx, &wid_str, StatusKind::Failed);
                return;
            }
        };

    // Wrap the entire execution in a timeout.
    let result = tokio::time::timeout(timeout, async {
        run_workflow_steps(&ctx, &wid, &workflow, &secrets, logger.as_ref()).await
    })
    .await;

    match result {
        Ok(Ok(())) => {
            // All steps succeeded.
            if let Err(e) = ctx.db.status_success(&wid_str) {
                error!(%e, workflow_id = %wid_str, "failed to set success status");
            }
            emit_status_event(&ctx, &wid_str, StatusKind::Success);
            info!(workflow_id = %wid_str, "workflow completed successfully");
        }
        Ok(Err(e)) => {
            // A step failed.
            error!(%e, workflow_id = %wid_str, "workflow failed");
            if let Err(e) = ctx.db.status_failed(&wid_str) {
                error!(%e, workflow_id = %wid_str, "failed to set failed status");
            }
            emit_status_event(&ctx, &wid_str, StatusKind::Failed);
        }
        Err(_) => {
            // Timed out.
            error!(workflow_id = %wid_str, timeout = ?timeout, "workflow timed out");
            if let Err(e) = ctx.db.status_timeout(&wid_str) {
                error!(%e, workflow_id = %wid_str, "failed to set timeout status");
            }
            emit_status_event(&ctx, &wid_str, StatusKind::Timeout);
        }
    }

    // Always destroy the workflow environment.
    if let Err(e) = ctx.engine.destroy_workflow(&wid).await {
        warn!(%e, workflow_id = %wid_str, "failed to destroy workflow environment");
    }

    // Close the logger.
    if let Err(e) = logger.close() {
        warn!(%e, workflow_id = %wid_str, "failed to close workflow logger");
    }
}

/// Run setup + all steps for a workflow.
async fn run_workflow_steps(
    ctx: &OrchestratorContext,
    wid: &WorkflowId,
    workflow: &spindle_models::Workflow,
    secrets: &[spindle_models::UnlockedSecret],
    logger: &dyn WorkflowLogger,
) -> Result<(), EngineError> {
    use spindle_models::log_line::StepStatus;

    // Setup the workflow environment (build Nix closure, create workspace).
    info!(workflow_id = %wid, "setting up workflow environment");
    ctx.engine.setup_workflow(wid, workflow, logger).await?;

    // Execute each step sequentially.
    for (step_idx, step) in workflow.steps.iter().enumerate() {
        info!(
            workflow_id = %wid,
            step_idx,
            name = step.name(),
            "executing step"
        );

        // Write control log: step start.
        let mut start_writer = logger.control_writer(step_idx, step.as_ref(), StepStatus::Start);
        let _ = start_writer.write_all(b"start");

        // Run the step.
        let step_result = ctx
            .engine
            .run_step(wid, workflow, step_idx, secrets, logger)
            .await;

        // Write control log: step end.
        let mut end_writer = logger.control_writer(step_idx, step.as_ref(), StepStatus::End);
        let _ = end_writer.write_all(b"end");

        // Propagate step failures.
        step_result?;

        info!(
            workflow_id = %wid,
            step_idx,
            name = step.name(),
            "step completed successfully"
        );
    }

    Ok(())
}

/// Emit a pipeline status event to the notifier and database.
fn emit_status_event(ctx: &OrchestratorContext, workflow_id: &str, status: StatusKind) {
    let payload = serde_json::json!({
        "type": "workflow_status",
        "workflowId": workflow_id,
        "status": status,
    });

    let payload_str = payload.to_string();

    // Insert into the events table.
    match ctx.db.insert_event("workflow_status", &payload_str) {
        Ok(event_id) => {
            // Broadcast to WebSocket clients.
            let event = spindle_db::events::Event {
                id: event_id,
                kind: "workflow_status".into(),
                payload: payload_str,
                created_at: chrono::Utc::now().to_rfc3339(),
            };
            ctx.notifier.notify(event);
        }
        Err(e) => {
            warn!(%e, workflow_id, "failed to insert status event");
        }
    }
}
