use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    form::{field, v_form},
    input::{Input, InputState},
    notification::NotificationType,
    select::{Select, SelectState},
    *,
};
use std::path::PathBuf;

use crate::{
    services::{
        database::traits::DatabaseType, ConnectionInfo, ConnectionsRepository, DatabaseManager,
        SslMode,
    },
    state::{add_connection, connect, delete_connection, update_connection},
};

#[allow(dead_code)]
pub enum ConnectionSavedEvent {
    ConnectionSaved,
    ConnectionSavedError { error: String },
}

impl EventEmitter<ConnectionSavedEvent> for ConnectionForm {}

/// Wrapper for DatabaseType to implement SelectItem.
#[derive(Debug, Clone, PartialEq)]
pub struct DatabaseTypeItem(pub DatabaseType);

impl gpui_component::select::SelectItem for DatabaseTypeItem {
    type Value = DatabaseType;

    fn title(&self) -> SharedString {
        self.0.display_name().into()
    }

    fn value(&self) -> &Self::Value {
        &self.0
    }
}

pub struct ConnectionForm {
    // Common fields
    name: Entity<InputState>,
    database_type: Entity<SelectState<Vec<DatabaseTypeItem>>>,

    // Server-based connection fields (PostgreSQL, MySQL, ClickHouse)
    hostname: Entity<InputState>,
    username: Entity<InputState>,
    password: Entity<InputState>,
    database: Entity<InputState>,
    port: Entity<InputState>,

    // File-based connection fields (SQLite, DuckDB)
    file_path: Entity<InputState>,
    read_only: bool,

    // Current state
    active_connection: Option<ConnectionInfo>,
    selected_type: DatabaseType,
    is_testing: bool,
}

impl ConnectionForm {
    pub fn view(
        connection: Option<ConnectionInfo>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| Self::new(connection, window, cx))
    }

    fn new(
        connection: Option<ConnectionInfo>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // Common fields
        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Name")
                .clean_on_escape()
        });

        // Database type selector
        let database_types = vec![
            DatabaseTypeItem(DatabaseType::PostgreSQL),
            DatabaseTypeItem(DatabaseType::MySQL),
            DatabaseTypeItem(DatabaseType::SQLite),
            DatabaseTypeItem(DatabaseType::ClickHouse),
            DatabaseTypeItem(DatabaseType::DuckDB),
        ];
        let database_type = cx.new(|cx| SelectState::new(database_types, None, window, cx));

        // Server-based fields
        let hostname = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Hostname")
                .clean_on_escape()
        });
        let username = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Username")
                .clean_on_escape()
        });
        let password = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("Password")
                .clean_on_escape()
        });
        let database = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Database")
                .clean_on_escape()
        });
        let port = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Port")
                .clean_on_escape()
        });

        // File-based fields
        let file_path = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("/path/to/database.db")
                .clean_on_escape()
        });

        let mut form = Self {
            name,
            database_type,
            hostname,
            username,
            password,
            database,
            port,
            file_path,
            read_only: false,
            active_connection: None,
            selected_type: DatabaseType::PostgreSQL,
            is_testing: false,
        };

        // Load existing connection if provided
        if let Some(conn) = connection {
            form.set_connection(conn, window, cx);
        }

        form
    }

    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self
            .name
            .update(cx, |this, cx| this.set_value("", window, cx));
        let _ = self
            .hostname
            .update(cx, |this, cx| this.set_value("", window, cx));
        let _ = self
            .username
            .update(cx, |this, cx| this.set_value("", window, cx));
        let _ = self
            .password
            .update(cx, |this, cx| this.set_value("", window, cx));
        let _ = self
            .database
            .update(cx, |this, cx| this.set_value("", window, cx));
        let _ = self
            .port
            .update(cx, |this, cx| this.set_value("", window, cx));
        let _ = self
            .file_path
            .update(cx, |this, cx| this.set_value("", window, cx));

        self.read_only = false;
        self.active_connection = None;
        self.selected_type = DatabaseType::PostgreSQL;

        cx.notify();
    }

    pub fn set_connection(
        &mut self,
        connection: ConnectionInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = self.name.update(cx, |this, cx| {
            this.set_value(connection.name.clone(), window, cx)
        });

        // Set the database type
        self.selected_type = connection.database_type;

        if connection.database_type.is_file_based() {
            // File-based database
            if let Some(path) = &connection.file_path {
                let _ = self.file_path.update(cx, |this, cx| {
                    this.set_value(path.display().to_string(), window, cx)
                });
            }
            self.read_only = connection.read_only;
        } else {
            // Server-based database
            let _ = self.hostname.update(cx, |this, cx| {
                this.set_value(connection.hostname.clone(), window, cx)
            });
            let _ = self.username.update(cx, |this, cx| {
                this.set_value(connection.username.clone(), window, cx)
            });
            let _ = self.password.update(cx, |this, cx| {
                this.set_value(connection.password.clone(), window, cx)
            });
            let _ = self.database.update(cx, |this, cx| {
                this.set_value(connection.database.clone(), window, cx)
            });
            let _ = self.port.update(cx, |this, cx| {
                this.set_value(connection.port.to_string(), window, cx)
            });
        }

        self.active_connection = Some(connection.clone());
        cx.notify();
    }

    fn connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(connection) = self.get_connection(window, cx) {
            connect(&connection, cx);
            self.clear(window, cx);
            cx.notify();
        }
    }

    fn get_connection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<ConnectionInfo> {
        let name = self.name.read(cx).value();

        if name.is_empty() {
            window.push_notification(
                (NotificationType::Error, "Connection name is required."),
                cx,
            );
            return None;
        }

        let db_type = self.selected_type;

        if db_type.is_file_based() {
            // File-based database (SQLite, DuckDB)
            let file_path_str = self.file_path.read(cx).value();

            if file_path_str.is_empty() {
                window.push_notification(
                    (NotificationType::Error, "Database file path is required."),
                    cx,
                );
                return None;
            }

            let file_path = PathBuf::from(file_path_str.to_string());

            if let Some(ref existing) = self.active_connection {
                Some(ConnectionInfo {
                    id: existing.id,
                    name: name.to_string(),
                    database_type: db_type,
                    hostname: String::new(),
                    username: String::new(),
                    password: String::new(),
                    database: String::new(),
                    port: 0,
                    ssl_mode: SslMode::default(),
                    file_path: Some(file_path),
                    read_only: self.read_only,
                })
            } else {
                Some(ConnectionInfo::new_file(
                    name.to_string(),
                    db_type,
                    file_path,
                    self.read_only,
                ))
            }
        } else {
            // Server-based database (PostgreSQL, MySQL, ClickHouse)
            let hostname = self.hostname.read(cx).value();
            let username = self.username.read(cx).value();
            let password = self.password.read(cx).value();
            let database = self.database.read(cx).value();
            let port = self.port.read(cx).value();

            // For editing: if password is empty, try to fetch from keychain
            let password = if password.is_empty() {
                if let Some(ref active) = self.active_connection {
                    ConnectionsRepository::get_connection_password(&active.id).unwrap_or_default()
                } else {
                    password.to_string()
                }
            } else {
                password.to_string()
            };

            // Validate inputs
            if hostname.is_empty()
                || username.is_empty()
                || password.is_empty()
                || database.is_empty()
                || port.is_empty()
            {
                window.push_notification(
                    (
                        NotificationType::Error,
                        "Not all fields have values. Please try again.",
                    ),
                    cx,
                );
                return None;
            }

            let port_num = match port.parse::<usize>() {
                Ok(num) => {
                    tracing::debug!("Successfully parsed: {}", num);
                    num
                }
                Err(e) => {
                    tracing::error!("Failed to parse integer: {}", e);
                    0
                }
            };

            if port_num < 1 || port_num > 65_535 {
                window.push_notification((NotificationType::Error, "Invalid port number."), cx);
                return None;
            }

            if let Some(ref existing) = self.active_connection {
                Some(ConnectionInfo {
                    id: existing.id,
                    name: name.to_string(),
                    database_type: db_type,
                    hostname: hostname.to_string(),
                    username: username.to_string(),
                    password: password.to_string(),
                    database: database.to_string(),
                    port: port_num,
                    ssl_mode: SslMode::Prefer,
                    file_path: None,
                    read_only: false,
                })
            } else {
                Some(ConnectionInfo::new_server(
                    name.to_string(),
                    db_type,
                    hostname.to_string(),
                    port_num,
                    username.to_string(),
                    password.to_string(),
                    database.to_string(),
                    SslMode::Prefer,
                ))
            }
        }
    }

    fn save_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(connection) = self.get_connection(window, cx) {
            add_connection(connection, cx);
            self.clear(window, cx);
        }
    }

    fn update_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(connection) = self.get_connection(window, cx) {
            update_connection(connection, cx);
        }
    }

    fn delete_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(connection) = self.active_connection.clone() {
            delete_connection(connection, cx);
            self.clear(window, cx);
        }
    }

    fn test_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_testing {
            return;
        }

        if let Some(connection) = self.get_connection(window, cx) {
            // Only PostgreSQL is fully supported for testing right now
            if connection.database_type != DatabaseType::PostgreSQL {
                let msg: SharedString = format!(
                    "Test connection not yet implemented for {}. Configuration saved.",
                    connection.database_type.display_name()
                )
                .into();
                window.push_notification((NotificationType::Warning, msg), cx);
                return;
            }

            self.is_testing = true;
            cx.notify();

            let connect_options = connection.to_pg_connect_options();
            let entity = cx.entity();

            cx.spawn_in(window, async move |_this, cx| {
                let result = DatabaseManager::test_connection_options(connect_options).await;

                let _ = cx.update(|window, cx| {
                    match result {
                        Ok(_) => {
                            window.push_notification(
                                (NotificationType::Success, "Connection successful!"),
                                cx,
                            );
                        }
                        Err(e) => {
                            let error_msg: SharedString =
                                format!("Connection failed: {}", e).into();
                            tracing::error!("{}", error_msg.clone());
                            window.push_notification((NotificationType::Error, error_msg), cx);
                        }
                    }

                    cx.update_entity(&entity, |form, cx| {
                        form.is_testing = false;
                        cx.notify();
                    });
                });
            })
            .detach();
        }
    }

    /// Render server-based connection fields (PostgreSQL, MySQL, ClickHouse).
    fn render_server_fields(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        v_form()
            .columns(2)
            .small()
            .child(
                field()
                    .label("Host/Socket")
                    .required(true)
                    .child(Input::new(&self.hostname)),
            )
            .child(
                field()
                    .label("Port")
                    .required(true)
                    .child(Input::new(&self.port)),
            )
            .child(
                field()
                    .label("Username")
                    .col_span(2)
                    .required(true)
                    .child(Input::new(&self.username)),
            )
            .child(
                field()
                    .col_span(2)
                    .label("Password")
                    .required(true)
                    .child(Input::new(&self.password)),
            )
            .child(
                field()
                    .col_span(2)
                    .label("Database")
                    .required(true)
                    .child(Input::new(&self.database)),
            )
    }

    /// Render file-based connection fields (SQLite, DuckDB).
    fn render_file_fields(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_form()
            .columns(1)
            .small()
            .child(
                field()
                    .label("Database File")
                    .required(true)
                    .description("Path to the database file")
                    .child(Input::new(&self.file_path)),
            )
            .child(
                field().label_indent(false).child(
                    Checkbox::new("read-only")
                        .label("Open in read-only mode")
                        .checked(self.read_only)
                        .on_click(cx.listener(|this, checked: &bool, _win, cx| {
                            this.read_only = *checked;
                            cx.notify();
                        })),
                ),
            )
    }
}

impl Render for ConnectionForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_editing = self.active_connection.is_some();
        let title = if is_editing {
            "Edit Connection"
        } else {
            "Add Connection"
        };

        // Update selected type from select state
        if let Some(selected_value) = self.database_type.read(cx).selected_value() {
            if *selected_value != self.selected_type {
                self.selected_type = *selected_value;
            }
        }

        // Render type-specific fields
        let type_specific_fields: AnyElement = if self.selected_type.is_file_based() {
            self.render_file_fields(cx).into_any_element()
        } else {
            self.render_server_fields(cx).into_any_element()
        };

        div()
            .mb_4()
            .child(div().text_3xl().mb_4().child(title))
            .child(
                v_form()
                    .columns(2)
                    .small()
                    .child(
                        field()
                            .col_span(2)
                            .label("Name")
                            .required(true)
                            .child(Input::new(&self.name)),
                    )
                    .child(
                        field()
                            .col_span(2)
                            .label("Database Type")
                            .child(Select::new(&self.database_type)),
                    ),
            )
            .child(div().my_4().child(type_specific_fields))
            .child(
                h_flex()
                    .mt_4()
                    .gap_2()
                    .child(
                        Button::new("test-connection")
                            .child("Test Connection")
                            .loading(self.is_testing)
                            .on_click(cx.listener(|this, _, win, cx| {
                                this.test_connection(win, cx)
                            })),
                    )
                    .when(!is_editing, |d| {
                        d.child(
                            Button::new("save-connection")
                                .primary()
                                .child("Save")
                                .on_click(cx.listener(|this, _, win, cx| {
                                    this.save_connection(win, cx)
                                })),
                        )
                    })
                    .when(is_editing, |d| {
                        d.child(
                            Button::new("delete-connection")
                                .child("Delete")
                                .danger()
                                .on_click(cx.listener(|_this, _, win, cx| {
                                    let entity = cx.entity();
                                    win.open_dialog(cx, move |dialog, _win, _cx| {
                                        let entity_clone = entity.clone();
                                        dialog
                                            .confirm()
                                            .child("Are you sure you want to delete this connection?")
                                            .on_ok(move |_, window, cx| {
                                                cx.update_entity(&entity_clone.clone(), |entity, cx| {
                                                    entity.delete_connection(window, cx);
                                                    cx.notify();
                                                });

                                                window.push_notification(
                                                    (NotificationType::Success, "Deleted"),
                                                    cx,
                                                );
                                                true
                                            })
                                    });
                                })),
                        )
                        .child(
                            Button::new("update-connection")
                                .primary()
                                .child("Update")
                                .on_click(cx.listener(|this, _, win, cx| {
                                    this.update_connection(win, cx)
                                })),
                        )
                        .child(
                            Button::new("connect")
                                .primary()
                                .child("Connect")
                                .on_click(cx.listener(|this, _, win, cx| this.connect(win, cx))),
                        )
                    }),
            )
            .text_sm()
    }
}
