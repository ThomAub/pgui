use crate::services::{DatabaseManager, DatabaseType};
use gpui::*;
use gpui::prelude::FluentBuilder;
use gpui_component::{
    ActiveTheme as _, Disableable, Icon, Sizable as _, StyledExt,
    button::{Button, ButtonVariants as _},
    input::{InputState, TextInput},
    label::Label,
    v_flex, h_flex,
};
use std::sync::Arc;

pub enum ConnectionEvent {
    Connected(Arc<DatabaseManager>),
    Disconnected,
    ConnectionError { field1: String },
}

impl EventEmitter<ConnectionEvent> for ConnectionsPanel {}

pub struct ConnectionsPanel {
    pub db_manager: Arc<DatabaseManager>,
    input_esc: Entity<InputState>,
    is_connected: bool,
    is_loading: bool,
    selected_db_type: DatabaseType,
}

impl ConnectionsPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let selected_db_type = DatabaseType::PostgreSQL;
        let input_esc = cx.new(|cx| {
            let mut i = InputState::new(window, cx)
                .placeholder(Self::get_placeholder_for_type(&selected_db_type))
                .clean_on_escape();

            i.set_value(Self::get_example_url_for_type(&selected_db_type), window, cx);
            i
        });

        Self {
            db_manager: Arc::new(DatabaseManager::new()),
            input_esc,
            is_connected: false,
            is_loading: false,
            selected_db_type,
        }
    }

    fn get_placeholder_for_type(db_type: &DatabaseType) -> &'static str {
        match db_type {
            DatabaseType::PostgreSQL => "postgres://user:pass@host:port/db",
            DatabaseType::SQLite => "path/to/database.db or sqlite://path/to/database.db",
            DatabaseType::ClickHouse => "http://user:pass@host:8123/database",
        }
    }

    fn get_example_url_for_type(db_type: &DatabaseType) -> &'static str {
        match db_type {
            DatabaseType::PostgreSQL => "postgres://test:test@localhost:5432/test",
            DatabaseType::SQLite => "test.db",
            DatabaseType::ClickHouse => "http://default:@localhost:8123/default",
        }
    }

    fn select_database_type(
        &mut self,
        db_type: DatabaseType,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selected_db_type = db_type.clone();
        self.input_esc.update(cx, |input, cx| {
            input.set_placeholder(Self::get_placeholder_for_type(&db_type), window, cx);
            input.set_value(Self::get_example_url_for_type(&db_type), window, cx);
        });
        cx.notify();
    }

    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    pub fn connect_to_database(
        &mut self,
        _: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_loading {
            return;
        }

        self.is_loading = true;
        cx.notify();

        let db_manager = self.db_manager.clone();
        let connection_url = self.input_esc.read(cx).value().clone();

        cx.spawn(async move |this: WeakEntity<ConnectionsPanel>, cx| {
            let result = db_manager.connect(&connection_url).await;

            this.update(cx, |this, cx| {
                this.is_loading = false;
                match result {
                    Ok(_) => {
                        this.is_connected = true;
                        cx.emit(ConnectionEvent::Connected(this.db_manager.clone()));
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to connect to database: {}", e);
                        eprintln!("{}", error_msg);
                        this.is_connected = false;
                        cx.emit(ConnectionEvent::ConnectionError { field1: error_msg });
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn disconnect_from_database(
        &mut self,
        _: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let db_manager = self.db_manager.clone();

        cx.spawn(async move |this, cx| {
            db_manager.disconnect().await;

            this.update(cx, |this, cx| {
                this.is_connected = false;
                cx.emit(ConnectionEvent::Disconnected);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    #[allow(dead_code)]
    pub fn is_connected(&self) -> bool {
        self.is_connected
    }

    fn render_connection_section(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let connection_button = if self.is_connected {
            Button::new("disconnect")
                .label("Disconnect")
                .icon(Icon::empty().path("icons/unplug.svg"))
                .small()
                .danger()
                .on_click(cx.listener(Self::disconnect_from_database))
        } else {
            Button::new("connect")
                .label(if self.is_loading {
                    "Connecting..."
                } else {
                    "Connect"
                })
                .icon(Icon::empty().path("icons/plug-zap.svg"))
                .small()
                .outline()
                .disabled(self.is_loading)
                .on_click(cx.listener(Self::connect_to_database))
        };

        let db_type_selector = h_flex()
            .gap_1()
            .child(
                Button::new("postgres")
                    .label("PostgreSQL")
                    .small()
                    .when(self.selected_db_type == DatabaseType::PostgreSQL, |b| b.primary())
                    .when(self.selected_db_type != DatabaseType::PostgreSQL, |b| b.ghost())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.select_database_type(DatabaseType::PostgreSQL, window, cx);
                    }))
            )
            .child(
                Button::new("sqlite")
                    .label("SQLite")
                    .small()
                    .when(self.selected_db_type == DatabaseType::SQLite, |b| b.primary())
                    .when(self.selected_db_type != DatabaseType::SQLite, |b| b.ghost())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.select_database_type(DatabaseType::SQLite, window, cx);
                    }))
            )
            .child(
                Button::new("clickhouse")
                    .label("ClickHouse")
                    .small()
                    .when(self.selected_db_type == DatabaseType::ClickHouse, |b| b.primary())
                    .when(self.selected_db_type != DatabaseType::ClickHouse, |b| b.ghost())
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.select_database_type(DatabaseType::ClickHouse, window, cx);
                    }))
            );

        v_flex()
            .gap_2()
            .p_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(Label::new("Database Connection").font_bold().text_sm())
            .child(Label::new("Database Type").text_xs())
            .child(db_type_selector)
            .child(Label::new("Connection URL").text_xs())
            .child(TextInput::new(&self.input_esc).cleanable())
            .child(connection_button)
    }

    fn render_connections_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_2()
            .p_3()
            .flex_1()
            .child(Label::new("Saved Connections").font_bold().text_sm())
            .child(
                Label::new("Feature coming soon...")
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            )
    }
}

impl Render for ConnectionsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().sidebar)
            .child(self.render_connection_section(cx))
            .child(self.render_connections_list(cx))
    }
}
