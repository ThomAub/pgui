use super::connections::ConnectionManager;
use super::editor::Editor;
use super::editor::EditorEvent;
use super::footer_bar::{FooterBar, FooterBarEvent};
use super::header_bar::HeaderBar;
#[cfg(feature = "keyboard-nav")]
use super::help_overlay::HelpOverlay;
use super::storage::{StorageBrowser, StorageManager as StorageManagerPanel};
use super::tables::{TableEvent, TablesTree};

#[cfg(feature = "keyboard-nav")]
use crate::keybindings::{editor as editor_actions, focus, global};
use crate::services::AppStore;
use crate::services::{ErrorResult, QueryExecutionResult, TableInfo};
use crate::state::{ConnectionState, ConnectionStatus, StorageConnectionStatus, StorageState};
use crate::workspace::agent::AgentPanel;
use crate::workspace::agent::AgentPanelEvent;
use crate::workspace::history::HistoryEvent;
use crate::workspace::history::HistoryPanel;
use crate::workspace::results::ResultsPanel;
use gpui::prelude::FluentBuilder as _;
use gpui::*;

use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::ActiveTheme;
use gpui_component::Root;
use gpui_component::resizable::{resizable_panel, v_resizable};
use gpui_component::spinner::Spinner;
use gpui_component::{h_flex, Icon, Selectable as _, Sizable as _};

/// Current workspace mode.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceMode {
    /// Database connection and query mode.
    Database,
    /// Storage browser mode (S3, etc.).
    Storage,
}

/// Currently focused panel in the workspace.
#[cfg(feature = "keyboard-nav")]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)]
pub enum FocusedPanel {
    Editor,
    Tables,
    Results,
    Agent,
    History,
    ConnectionList,
    StorageBrowser,
}

pub struct Workspace {
    /// Current workspace mode.
    mode: WorkspaceMode,
    /// Database connection state.
    connection_state: ConnectionStatus,
    /// Storage connection state.
    storage_state: StorageConnectionStatus,

    // Header and footer
    header_bar: Entity<HeaderBar>,
    footer_bar: Entity<FooterBar>,

    // Database components
    tables_tree: Entity<TablesTree>,
    editor: Entity<Editor>,
    agent_panel: Entity<AgentPanel>,
    history_panel: Entity<HistoryPanel>,
    connection_manager: Entity<ConnectionManager>,
    results_panel: Entity<ResultsPanel>,

    // Storage components
    storage_manager: Entity<StorageManagerPanel>,
    storage_browser: Entity<StorageBrowser>,

    _subscriptions: Vec<Subscription>,
    show_tables: bool,
    show_agent: bool,
    show_history: bool,

    /// Whether to show the help overlay.
    #[cfg(feature = "keyboard-nav")]
    show_help: bool,
    /// Currently focused panel.
    #[cfg(feature = "keyboard-nav")]
    focused_panel: Option<FocusedPanel>,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let header_bar = HeaderBar::view(window, cx);
        let footer_bar = FooterBar::view(window, cx);
        let tables_tree = TablesTree::view(window, cx);
        let agent_panel = AgentPanel::view(window, cx);
        let history_panel = HistoryPanel::view(window, cx);
        let editor = Editor::view(window, cx);
        let results_panel = ResultsPanel::view(window, cx);
        let connection_manager = ConnectionManager::view(window, cx);

        // Storage components
        let storage_manager = StorageManagerPanel::view(window, cx);
        let storage_browser = StorageBrowser::view(window, cx);

        let _subscriptions = vec![
            cx.observe_global::<ConnectionState>(move |this, cx| {
                this.connection_state = cx.global::<ConnectionState>().connection_state.clone();
                cx.notify();
            }),
            cx.observe_global::<StorageState>(move |this, cx| {
                this.storage_state = cx.global::<StorageState>().connection_status.clone();
                cx.notify();
            }),
            cx.subscribe(&editor, |this, _, event: &EditorEvent, cx| match event {
                EditorEvent::ExecuteQuery(query) => {
                    this.execute_query(query.clone(), cx);
                }
            }),
            cx.subscribe(&tables_tree, |this, _, event: &TableEvent, cx| {
                this.handle_table_event(event, cx);
            }),
            cx.subscribe(&footer_bar, |this, _, event: &FooterBarEvent, cx| {
                match event {
                    FooterBarEvent::ToggleTables(show) => {
                        this.show_tables = *show;
                    }
                    FooterBarEvent::ToggleAgent(show) => {
                        this.show_agent = *show;
                    }
                    FooterBarEvent::ToggleHistory(show) => {
                        this.show_history = *show;
                    }
                }
                cx.notify();
            }),
            // Subscribe to history panel events
            cx.subscribe_in(
                &history_panel,
                window,
                |this, _, event: &HistoryEvent, win, cx| match event {
                    HistoryEvent::LoadQuery(sql) => {
                        this.load_query_into_editor(sql.clone(), win, cx);
                    }
                },
            ),
            cx.subscribe_in(
                &agent_panel,
                window,
                |this, _, event: &AgentPanelEvent, window, cx| match event {
                    AgentPanelEvent::RunQuery(sql) => {
                        // Load into editor and execute
                        this.load_query_into_editor(sql.clone().to_string(), window, cx);
                        this.execute_query(sql.clone().to_string(), cx);
                    }
                },
            ),
        ];

        Self {
            mode: WorkspaceMode::Database,
            header_bar,
            footer_bar,
            connection_manager,
            tables_tree,
            editor,
            agent_panel,
            history_panel,
            results_panel,
            storage_manager,
            storage_browser,
            _subscriptions,
            connection_state: ConnectionStatus::Disconnected,
            storage_state: StorageConnectionStatus::Disconnected,
            show_tables: true,
            show_agent: false,
            show_history: false,
            #[cfg(feature = "keyboard-nav")]
            show_help: false,
            #[cfg(feature = "keyboard-nav")]
            focused_panel: None,
        }
    }

    // ========================================================================
    // Keyboard Action Handlers
    // ========================================================================

    #[cfg(feature = "keyboard-nav")]
    fn on_switch_to_database(&mut self, _: &global::SwitchToDatabase, _window: &mut Window, cx: &mut Context<Self>) {
        self.mode = WorkspaceMode::Database;
        cx.notify();
    }

    #[cfg(feature = "keyboard-nav")]
    fn on_switch_to_storage(&mut self, _: &global::SwitchToStorage, _window: &mut Window, cx: &mut Context<Self>) {
        self.mode = WorkspaceMode::Storage;
        cx.notify();
    }

    #[cfg(feature = "keyboard-nav")]
    fn on_toggle_sidebar(&mut self, _: &global::ToggleSidebar, _window: &mut Window, cx: &mut Context<Self>) {
        self.show_tables = !self.show_tables;
        // Update footer bar state
        self.footer_bar.update(cx, |footer, cx| {
            footer.set_show_tables(self.show_tables, cx);
        });
        cx.notify();
    }

    #[cfg(feature = "keyboard-nav")]
    fn on_toggle_right_panel(&mut self, _: &global::ToggleRightPanel, _window: &mut Window, cx: &mut Context<Self>) {
        // Toggle between agent and history (or close if one is open)
        if self.show_agent {
            self.show_agent = false;
        } else if self.show_history {
            self.show_history = false;
        } else {
            // Open agent panel by default
            self.show_agent = true;
        }
        cx.notify();
    }

    #[cfg(feature = "keyboard-nav")]
    fn on_toggle_history(&mut self, _: &global::ToggleHistory, _window: &mut Window, cx: &mut Context<Self>) {
        self.show_history = !self.show_history;
        if self.show_history {
            self.show_agent = false; // They're mutually exclusive
        }
        // Update footer bar state
        self.footer_bar.update(cx, |footer, cx| {
            footer.set_show_history(self.show_history, cx);
        });
        cx.notify();
    }

    #[cfg(feature = "keyboard-nav")]
    fn on_toggle_agent(&mut self, _: &global::ToggleAgent, _window: &mut Window, cx: &mut Context<Self>) {
        self.show_agent = !self.show_agent;
        if self.show_agent {
            self.show_history = false; // They're mutually exclusive
        }
        // Update footer bar state
        self.footer_bar.update(cx, |footer, cx| {
            footer.set_show_agent(self.show_agent, cx);
        });
        cx.notify();
    }

    #[cfg(feature = "keyboard-nav")]
    fn on_show_help(&mut self, _: &global::ShowHelp, _window: &mut Window, cx: &mut Context<Self>) {
        self.show_help = true;
        cx.notify();
    }

    #[cfg(feature = "keyboard-nav")]
    fn on_hide_help(&mut self, _: &global::HideHelp, _window: &mut Window, cx: &mut Context<Self>) {
        self.show_help = false;
        cx.notify();
    }

    #[cfg(feature = "keyboard-nav")]
    fn on_escape(&mut self, _: &global::Escape, _window: &mut Window, cx: &mut Context<Self>) {
        // Close help overlay if open
        if self.show_help {
            self.show_help = false;
            cx.notify();
            return;
        }
        // Clear focus
        self.focused_panel = None;
        cx.notify();
    }

    #[cfg(feature = "keyboard-nav")]
    fn on_focus_tables(&mut self, _: &focus::FocusTables, _window: &mut Window, cx: &mut Context<Self>) {
        self.focused_panel = Some(FocusedPanel::Tables);
        self.show_tables = true;
        cx.notify();
    }

    #[cfg(feature = "keyboard-nav")]
    fn on_focus_results(&mut self, _: &focus::FocusResults, _window: &mut Window, cx: &mut Context<Self>) {
        self.focused_panel = Some(FocusedPanel::Results);
        cx.notify();
    }

    #[cfg(feature = "keyboard-nav")]
    fn on_focus_agent(&mut self, _: &focus::FocusAgent, _window: &mut Window, cx: &mut Context<Self>) {
        self.focused_panel = Some(FocusedPanel::Agent);
        self.show_agent = true;
        self.show_history = false;
        cx.notify();
    }

    #[cfg(feature = "keyboard-nav")]
    fn on_focus_history(&mut self, _: &focus::FocusHistory, _window: &mut Window, cx: &mut Context<Self>) {
        self.focused_panel = Some(FocusedPanel::History);
        self.show_history = true;
        self.show_agent = false;
        cx.notify();
    }

    #[cfg(feature = "keyboard-nav")]
    fn on_focus_editor(&mut self, _: &editor_actions::FocusEditor, _window: &mut Window, cx: &mut Context<Self>) {
        self.focused_panel = Some(FocusedPanel::Editor);
        cx.notify();
    }

    #[cfg(feature = "keyboard-nav")]
    fn on_execute_query(&mut self, _: &editor_actions::ExecuteQuery, _window: &mut Window, cx: &mut Context<Self>) {
        // Trigger query execution from editor
        self.editor.update(cx, |editor, cx| {
            editor.do_execute_query(cx);
        });
    }

    #[cfg(feature = "keyboard-nav")]
    fn on_format_query(&mut self, _: &editor_actions::FormatQuery, window: &mut Window, cx: &mut Context<Self>) {
        // Trigger format from editor
        self.editor.update(cx, |editor, cx| {
            editor.do_format_query(window, cx);
        });
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn load_query_into_editor(&mut self, sql: String, window: &mut Window, cx: &mut App) {
        self.editor.update(cx, |editor, cx| {
            editor.set_query(sql, window, cx);
        });
    }

    fn execute_query(&mut self, query: String, cx: &mut Context<Self>) {
        // Set editor to executing state
        self.editor.update(cx, |editor, cx| {
            editor.set_executing(true, cx);
            cx.notify();
        });

        tracing::debug!("execute_query");

        // Get database manager from global state
        let db_manager = cx.global::<ConnectionState>().db_manager.clone();
        tracing::debug!("execute_query - db_manager");
        let active_connection = cx.global::<ConnectionState>().active_connection.clone();
        tracing::debug!("execute_query - active_connection");

        cx.spawn(async move |this, cx| {
            tracing::debug!("execute_query spawn - before execute_query_enhanced");
            let result = db_manager.execute_query_enhanced(&query).await;
            tracing::debug!("execute_query_enhanced result");
            // Extract execution info before moving result
            let (execution_time_ms, rows_affected) = match &result {
                QueryExecutionResult::Modified(modified) => (
                    Some(modified.execution_time_ms as i64),
                    Some(modified.rows_affected as i64),
                ),
                QueryExecutionResult::Select(r) => (Some(r.execution_time_ms as i64), None),
                QueryExecutionResult::Error(err) => (Some(err.execution_time_ms as i64), None),
            };

            this.update(cx, |this, cx| {
                // Update results panel
                this.results_panel.update(cx, |results, cx| {
                    results.update_result(result, cx);
                });

                // Set editor back to normal state
                this.editor.update(cx, |editor, cx| {
                    editor.set_executing(false, cx);
                });

                cx.notify();
            })
            .ok();

            if let Some(conn) = active_connection {
                if let Ok(store) = AppStore::singleton().await {
                    let _ = store
                        .history()
                        .record(
                            &conn.id,
                            &query.clone(),
                            execution_time_ms.unwrap_or(0),
                            rows_affected,
                            true,
                            None,
                        )
                        .await;
                }
            }
        })
        .detach();
    }

    fn handle_table_event(&mut self, event: &TableEvent, cx: &mut Context<Self>) {
        match event {
            TableEvent::TableSelected(table) => {
                self.show_table_columns(table.clone(), cx);
            }
        }
    }

    fn show_table_columns(&mut self, table: TableInfo, cx: &mut Context<Self>) {
        // Get database manager from global state
        let db_manager = cx.global::<ConnectionState>().db_manager.clone();

        cx.spawn(async move |this, cx| {
            let result = db_manager
                .get_table_columns(&table.table_name, &table.table_schema)
                .await;

            this.update(cx, |this, cx| {
                match result {
                    Ok(query_result) => {
                        this.results_panel.update(cx, |results, cx| {
                            results.update_result(query_result, cx);
                        });
                    }
                    Err(e) => {
                        this.results_panel.update(cx, |results, cx| {
                            results.update_result(
                                QueryExecutionResult::Error(ErrorResult {
                                    execution_time_ms: 0,
                                    message: format!("Failed to load table columns: {}", e),
                                }),
                                cx,
                            );
                        });
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Render the mode tabs for switching between Database and Storage.
    fn render_mode_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_db_mode = self.mode == WorkspaceMode::Database;
        let is_storage_mode = self.mode == WorkspaceMode::Storage;

        h_flex()
            .gap_1()
            .p_1()
            .bg(cx.theme().title_bar)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("mode-database")
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(Icon::empty().path("icons/database.svg").size_4())
                            .child("Database"),
                    )
                    .small()
                    .ghost()
                    .selected(is_db_mode)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.mode = WorkspaceMode::Database;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("mode-storage")
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(Icon::empty().path("icons/cloud-download.svg").size_4())
                            .child("Storage"),
                    )
                    .small()
                    .ghost()
                    .selected(is_storage_mode)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.mode = WorkspaceMode::Storage;
                        cx.notify();
                    })),
            )
    }

    fn render_database_disconnected(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("connection-manager")
            .flex()
            .flex_1()
            .bg(cx.theme().background)
            .child(self.connection_manager.clone())
    }

    fn render_database_connected(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let sidebar = div()
            .id("connected-sidebar")
            .flex()
            .flex_col()
            .h_full()
            .border_color(cx.theme().border)
            .border_r_1()
            .min_w(px(300.0))
            .child(self.tables_tree.clone());

        let agent = div()
            .id("connected-agent")
            .flex()
            .flex_col()
            .h_full()
            .w(px(400.))
            .border_color(cx.theme().border)
            .border_l_1()
            .child(self.agent_panel.clone());

        let history = div()
            .id("connected-history")
            .flex()
            .flex_col()
            .h_full()
            .w(px(400.))
            .border_color(cx.theme().border)
            .border_l_1()
            .child(self.history_panel.clone());

        let main = div()
            .id("connected-main")
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .w_full()
            .overflow_hidden()
            .child(
                v_resizable("resizable-results")
                    .child(
                        resizable_panel()
                            .size(px(400.))
                            .size_range(px(200.)..px(800.))
                            .child(self.editor.clone()),
                    )
                    .child(
                        resizable_panel()
                            .size(px(200.))
                            .child(self.results_panel.clone()),
                    ),
            );

        div()
            .id("connected-content")
            .flex()
            .flex_row()
            .flex_1()
            .h_full()
            .bg(cx.theme().background)
            .when(self.show_tables, |d| d.child(sidebar))
            .child(main)
            .when(self.show_agent, |d| d.child(agent))
            .when(self.show_history, |d| d.child(history))
    }

    fn render_storage_disconnected(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("storage-manager")
            .flex()
            .flex_1()
            .bg(cx.theme().background)
            .child(self.storage_manager.clone())
    }

    fn render_storage_connected(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("storage-browser")
            .flex()
            .flex_1()
            .bg(cx.theme().background)
            .child(self.storage_browser.clone())
    }

    fn render_loading(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("loading-content")
            .flex()
            .flex_grow()
            .bg(cx.theme().background)
            .justify_center()
            .items_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .child(Spinner::new())
                    .child("Loading"),
            )
    }

    fn render_content(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        match self.mode {
            WorkspaceMode::Database => match self.connection_state {
                ConnectionStatus::Disconnected => self.render_database_disconnected(cx),
                ConnectionStatus::Connected => self.render_database_connected(cx),
                ConnectionStatus::Disconnecting | ConnectionStatus::Connecting => {
                    self.render_loading(cx)
                }
            },
            WorkspaceMode::Storage => match self.storage_state {
                StorageConnectionStatus::Disconnected => self.render_storage_disconnected(cx),
                StorageConnectionStatus::Connected => self.render_storage_connected(cx),
                StorageConnectionStatus::Disconnecting | StorageConnectionStatus::Connecting => {
                    self.render_loading(cx)
                }
            },
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = self.render_content(cx);

        #[cfg(feature = "keyboard-nav")]
        let show_help = self.show_help;
        #[cfg(not(feature = "keyboard-nav"))]
        let show_help = false;

        let root = div()
            .flex()
            .flex_col()
            .size_full();

        // Register keyboard action handlers (feature-gated)
        #[cfg(feature = "keyboard-nav")]
        let root = root
            .on_action(cx.listener(Self::on_switch_to_database))
            .on_action(cx.listener(Self::on_switch_to_storage))
            .on_action(cx.listener(Self::on_toggle_sidebar))
            .on_action(cx.listener(Self::on_toggle_right_panel))
            .on_action(cx.listener(Self::on_toggle_history))
            .on_action(cx.listener(Self::on_toggle_agent))
            .on_action(cx.listener(Self::on_show_help))
            .on_action(cx.listener(Self::on_hide_help))
            .on_action(cx.listener(Self::on_escape))
            .on_action(cx.listener(Self::on_focus_tables))
            .on_action(cx.listener(Self::on_focus_results))
            .on_action(cx.listener(Self::on_focus_agent))
            .on_action(cx.listener(Self::on_focus_history))
            .on_action(cx.listener(Self::on_focus_editor))
            .on_action(cx.listener(Self::on_execute_query))
            .on_action(cx.listener(Self::on_format_query));

        let root = root
            .child(self.header_bar.clone())
            .child(self.render_mode_tabs(cx))
            .child(content)
            .child(self.footer_bar.clone())
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_notification_layer(window, cx));

        // Help overlay (rendered on top when visible)
        #[cfg(feature = "keyboard-nav")]
        let root = if show_help {
            root.child(HelpOverlay::new())
        } else {
            root
        };

        root
    }
}
