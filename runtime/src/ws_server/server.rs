use super::*;

pub struct WsServer {
    pub(super) listener: TcpListener,
    pub(super) dispatcher: crate::engine_dispatch::EngineDispatcher,
    pub(super) ark_host: Arc<ArkHost>,
    pub(super) auth_token: Arc<String>,
    pub(super) command_bus: Arc<CommandBus>,
    pub(super) pomodoro_host: Arc<PomodoroHost>,
    pub(super) dictation_host: Arc<DictationHost>,
    pub(super) agents: Arc<tokio::sync::OnceCell<Arc<AgentsService>>>,
    pub(super) agent_events: tokio::sync::broadcast::Sender<serde_json::Value>,
    pub(super) protocol_usage: Arc<ProtocolUsageStore>,
    pub(super) correlation_id: Arc<String>,
    pub(super) lifecycle: Arc<WsLifecycle>,
    pub(super) desktop_authority: Arc<crate::desktop_authority::DesktopAuthorityRegistry>,
    pub(super) snapshots: Arc<crate::package_worker_broker::SnapshotRegistry>,
    pub(super) grants: Arc<GrantAuthorityRegistry>,
}
impl WsServer {
    /// Биндит TcpListener на 127.0.0.1 + случайный свободный порт.
    /// `data_dir` — куда писать persisted pomodoro state.
    #[allow(clippy::too_many_arguments)]
    pub async fn bind(
        ark_host: Arc<ArkHost>,
        auth_token: String,
        data_dir: std::path::PathBuf,
        app_index: Arc<AppIndex>,
        file_index: Arc<FileIndex>,
        usage_diagnostics: Arc<UsageTrackerDiagnosticsState>,
        protocol_usage: Arc<ProtocolUsageStore>,
        package_service: Arc<PackageService>,
        correlation_id: String,
    ) -> Result<Self, WsServerError> {
        let addr: SocketAddr = "127.0.0.1:0"
            .parse()
            .expect("hardcoded socket literal is always valid");
        let listener = TcpListener::bind(addr).await?;
        let (agent_events, _) = tokio::sync::broadcast::channel(512);
        let command_bus = Arc::new(CommandBus::new());
        let pomodoro_host = PomodoroHost::new(data_dir.clone());
        let dictation_host = DictationHost::new();
        let agents = Arc::new(tokio::sync::OnceCell::new());
        let agents_data_dir = Arc::new(data_dir.clone());
        let rpc_diagnostics: SharedRpcDiagnostics = Arc::new(RpcDiagnostics::new());
        let manager_state = ManagerState::new(data_dir.clone());
        let store_catalog = StoreCatalogService::open_compiled(&data_dir)
            .ok()
            .map(Arc::new);
        let snapshots = Arc::new(crate::package_worker_broker::SnapshotRegistry::new());
        let grants = Arc::new(GrantAuthorityRegistry::with_data_dir(data_dir.clone()));
        let desktop_authority = Arc::new(crate::desktop_authority::DesktopAuthorityRegistry::new());
        let dispatcher = make_dispatcher(
            ark_host.clone(),
            command_bus.clone(),
            pomodoro_host.clone(),
            dictation_host.clone(),
            app_index.clone(),
            file_index.clone(),
            agents.clone(),
            agents_data_dir.clone(),
            agent_events.clone(),
            usage_diagnostics.clone(),
            rpc_diagnostics.clone(),
            protocol_usage.clone(),
            package_service.clone(),
            store_catalog,
            snapshots.clone(),
            grants.clone(),
            desktop_authority.clone(),
            manager_state.clone(),
            Arc::new(correlation_id.clone()),
        );
        let lifecycle = Arc::new(WsLifecycle::default());
        Ok(WsServer {
            listener,
            dispatcher,
            ark_host,
            auth_token: Arc::new(auth_token),
            command_bus,
            pomodoro_host,
            dictation_host,
            agents,
            agent_events,
            protocol_usage,
            correlation_id: Arc::new(correlation_id),
            lifecycle,
            desktop_authority,
            snapshots,
            grants,
        })
    }

    #[allow(
        clippy::result_large_err,
        reason = "public API returns the local server error enum"
    )]
    pub fn local_addr(&self) -> Result<SocketAddr, WsServerError> {
        Ok(self.listener.local_addr()?)
    }

    pub fn port(&self) -> u16 {
        self.listener
            .local_addr()
            .map(|a| a.port())
            .unwrap_or_default()
    }

    pub fn agents_handle(&self) -> Arc<tokio::sync::OnceCell<Arc<AgentsService>>> {
        self.agents.clone()
    }

    pub fn dispatcher(&self) -> crate::engine_dispatch::EngineDispatcher {
        self.dispatcher.clone()
    }

    pub fn registration_count(&self) -> usize {
        self.command_bus.registration_count_sync()
    }

    pub fn command_bus_handle(&self) -> Arc<CommandBus> {
        self.command_bus.clone()
    }

    pub fn desktop_authority(&self) -> Arc<crate::desktop_authority::DesktopAuthorityRegistry> {
        self.desktop_authority.clone()
    }

    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }

    pub fn grant_count(&self) -> usize {
        self.grants.len()
    }

    pub fn snapshots_handle(&self) -> Arc<crate::package_worker_broker::SnapshotRegistry> {
        self.snapshots.clone()
    }

    pub fn grants_handle(&self) -> Arc<GrantAuthorityRegistry> {
        self.grants.clone()
    }

    pub fn shutdown_handle(&self) -> WsShutdownHandle {
        WsShutdownHandle {
            lifecycle: self.lifecycle.clone(),
        }
    }
}

/// The accept loop is itself joined by the runtime owner; every connection
/// task is inserted into the server registry before admission is released.
fn make_dispatcher(
    ark_host: Arc<ArkHost>,
    command_bus: Arc<CommandBus>,
    pomodoro_host: Arc<PomodoroHost>,
    dictation_host: Arc<DictationHost>,
    app_index: Arc<AppIndex>,
    file_index: Arc<FileIndex>,
    agents: Arc<tokio::sync::OnceCell<Arc<AgentsService>>>,
    agents_data_dir: Arc<std::path::PathBuf>,
    agent_events: tokio::sync::broadcast::Sender<serde_json::Value>,
    usage_diagnostics: Arc<UsageTrackerDiagnosticsState>,
    rpc_diagnostics: SharedRpcDiagnostics,
    protocol_usage: Arc<ProtocolUsageStore>,
    package_service: Arc<PackageService>,
    store_catalog: Option<Arc<StoreCatalogService>>,
    snapshots: Arc<crate::package_worker_broker::SnapshotRegistry>,
    grants: Arc<GrantAuthorityRegistry>,
    desktop_authority: Arc<crate::desktop_authority::DesktopAuthorityRegistry>,
    manager_state: ManagerState,
    correlation_id: Arc<String>,
) -> crate::engine_dispatch::EngineDispatcher {
    let cleanup_bus = command_bus.clone();
    let handler: crate::engine_dispatch::DispatchHandler =
        Arc::new(move |request: crate::engine_dispatch::DispatchRequest| {
            let ark_host = ark_host.clone();
            let command_bus = command_bus.clone();
            let pomodoro_host = pomodoro_host.clone();
            let dictation_host = dictation_host.clone();
            let app_index = app_index.clone();
            let file_index = file_index.clone();
            let agents = agents.clone();
            let agents_data_dir = agents_data_dir.clone();
            let agent_events = agent_events.clone();
            let usage_diagnostics = usage_diagnostics.clone();
            let rpc_diagnostics = rpc_diagnostics.clone();
            let protocol_usage = protocol_usage.clone();
            let package_service = package_service.clone();
            let store_catalog = store_catalog.clone();
            let snapshots = snapshots.clone();
            let grants = grants.clone();
            let desktop_authority = desktop_authority.clone();
            let manager_state = manager_state.clone();
            let correlation_id = correlation_id.clone();
            let connection_id = request.client.connection_id.unwrap_or(0);
            Box::pin(async move {
                Ok(dispatch_operation(
                    request,
                    ark_host,
                    command_bus,
                    pomodoro_host,
                    dictation_host,
                    app_index,
                    file_index,
                    agents,
                    agents_data_dir,
                    agent_events,
                    usage_diagnostics,
                    rpc_diagnostics,
                    protocol_usage,
                    package_service,
                    store_catalog,
                    snapshots,
                    grants,
                    desktop_authority,
                    manager_state,
                    correlation_id,
                    connection_id,
                )
                .await)
            })
        });
    let cleanup: crate::engine_dispatch::CleanupHandler = Arc::new(move |client_id| {
        if let Some(snapshot) = cleanup_bus.unregister_all_sync(client_id) {
            cleanup_bus.broadcast_changed_snapshot(snapshot);
        }
    });
    crate::engine_dispatch::EngineDispatcher::with_cleanup(handler, cleanup)
}
