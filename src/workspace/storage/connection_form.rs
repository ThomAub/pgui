//! Storage connection form for S3 and other blob storage services.

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
    services::database::storage::{StorageConfig, StorageParams, StorageType},
    state::{
        add_storage_connection, delete_storage_connection, storage_connect,
        update_storage_connection,
    },
};

/// Events emitted by the storage connection form.
#[allow(dead_code)]
pub enum StorageConnectionFormEvent {
    Saved,
    SaveError { error: String },
    Closed,
}

impl EventEmitter<StorageConnectionFormEvent> for StorageConnectionForm {}

/// Form for creating and editing storage connections.
pub struct StorageConnectionForm {
    // Common fields
    name: Entity<InputState>,
    storage_type: Entity<SelectState<StorageTypeItem>>,

    // S3 fields
    endpoint: Entity<InputState>,
    region: Entity<InputState>,
    bucket: Entity<InputState>,
    access_key_id: Entity<InputState>,
    secret_access_key: Entity<InputState>,
    path_style: bool,
    allow_anonymous: bool,

    // Local filesystem fields
    root_path: Entity<InputState>,

    // Current state
    active_connection: Option<StorageConfig>,
    selected_type: StorageType,
    is_testing: bool,
}

/// Wrapper for StorageType to implement SelectItem.
#[derive(Debug, Clone, PartialEq)]
pub struct StorageTypeItem(pub StorageType);

impl gpui_component::select::SelectItem for StorageTypeItem {
    type Value = StorageType;

    fn title(&self) -> SharedString {
        self.0.display_name().into()
    }

    fn value(&self) -> &Self::Value {
        &self.0
    }
}

impl StorageConnectionForm {
    /// Create a new storage connection form.
    pub fn view(
        connection: Option<StorageConfig>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|cx| Self::new(connection, window, cx))
    }

    fn new(
        connection: Option<StorageConfig>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // Create input states
        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Connection Name")
                .clean_on_escape()
        });
        let endpoint = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("https://s3.amazonaws.com (leave blank for AWS)")
                .clean_on_escape()
        });
        let region = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("us-east-1")
                .clean_on_escape()
        });
        let bucket = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("my-bucket")
                .clean_on_escape()
        });
        let access_key_id = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("AKIAIOSFODNN7EXAMPLE")
                .clean_on_escape()
        });
        let secret_access_key = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .placeholder("Secret Access Key")
                .clean_on_escape()
        });
        let root_path = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("/path/to/directory")
                .clean_on_escape()
        });

        // Storage type selector
        let storage_types = vec![
            StorageTypeItem(StorageType::S3),
            StorageTypeItem(StorageType::LocalFs),
        ];
        let storage_type = cx.new(|cx| SelectState::new(storage_types, None, window, cx));

        let mut form = Self {
            name,
            storage_type,
            endpoint,
            region,
            bucket,
            access_key_id,
            secret_access_key,
            path_style: false,
            allow_anonymous: false,
            root_path,
            active_connection: None,
            selected_type: StorageType::S3,
            is_testing: false,
        };

        // Load existing connection if provided
        if let Some(conn) = connection {
            form.set_connection(conn, window, cx);
        }

        form
    }

    /// Clear the form.
    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = self
            .name
            .update(cx, |this, cx| this.set_value("", window, cx));
        let _ = self
            .endpoint
            .update(cx, |this, cx| this.set_value("", window, cx));
        let _ = self
            .region
            .update(cx, |this, cx| this.set_value("", window, cx));
        let _ = self
            .bucket
            .update(cx, |this, cx| this.set_value("", window, cx));
        let _ = self
            .access_key_id
            .update(cx, |this, cx| this.set_value("", window, cx));
        let _ = self
            .secret_access_key
            .update(cx, |this, cx| this.set_value("", window, cx));
        let _ = self
            .root_path
            .update(cx, |this, cx| this.set_value("", window, cx));

        self.path_style = false;
        self.allow_anonymous = false;
        self.active_connection = None;
        self.selected_type = StorageType::S3;

        cx.notify();
    }

    /// Set the form to edit an existing connection.
    pub fn set_connection(
        &mut self,
        config: StorageConfig,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = self.name.update(cx, |this, cx| {
            this.set_value(config.name.clone(), window, cx)
        });

        self.selected_type = config.storage_type;

        match &config.params {
            StorageParams::S3 {
                endpoint,
                region,
                bucket,
                access_key_id,
                path_style,
                allow_anonymous,
                ..
            } => {
                let _ = self.endpoint.update(cx, |this, cx| {
                    this.set_value(endpoint.clone().unwrap_or_default(), window, cx)
                });
                let _ = self.region.update(cx, |this, cx| {
                    this.set_value(region.clone(), window, cx)
                });
                let _ = self.bucket.update(cx, |this, cx| {
                    this.set_value(bucket.clone(), window, cx)
                });
                let _ = self.access_key_id.update(cx, |this, cx| {
                    this.set_value(access_key_id.clone().unwrap_or_default(), window, cx)
                });
                self.path_style = *path_style;
                self.allow_anonymous = *allow_anonymous;
            }
            StorageParams::LocalFs { root_path } => {
                let _ = self.root_path.update(cx, |this, cx| {
                    this.set_value(root_path.display().to_string(), window, cx)
                });
            }
            _ => {}
        }

        self.active_connection = Some(config);
        cx.notify();
    }

    /// Get the current configuration from form values.
    fn get_config(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<StorageConfig> {
        let name = self.name.read(cx).value().to_string();

        if name.is_empty() {
            window.push_notification(
                (NotificationType::Error, "Connection name is required."),
                cx,
            );
            return None;
        }

        let params = match self.selected_type {
            StorageType::S3 => {
                let bucket = self.bucket.read(cx).value().to_string();
                if bucket.is_empty() {
                    window.push_notification(
                        (NotificationType::Error, "Bucket name is required."),
                        cx,
                    );
                    return None;
                }

                let endpoint = self.endpoint.read(cx).value().to_string();
                let region = self.region.read(cx).value().to_string();
                let access_key_id = self.access_key_id.read(cx).value().to_string();

                StorageParams::S3 {
                    endpoint: if endpoint.is_empty() {
                        None
                    } else {
                        Some(endpoint)
                    },
                    region: if region.is_empty() {
                        "us-east-1".to_string()
                    } else {
                        region
                    },
                    bucket,
                    access_key_id: if access_key_id.is_empty() {
                        None
                    } else {
                        Some(access_key_id)
                    },
                    path_style: self.path_style,
                    allow_anonymous: self.allow_anonymous,
                    extra_options: std::collections::HashMap::new(),
                }
            }
            StorageType::LocalFs => {
                let path_str = self.root_path.read(cx).value().to_string();
                if path_str.is_empty() {
                    window.push_notification(
                        (NotificationType::Error, "Root path is required."),
                        cx,
                    );
                    return None;
                }

                StorageParams::LocalFs {
                    root_path: PathBuf::from(path_str),
                }
            }
            _ => {
                window.push_notification(
                    (
                        NotificationType::Error,
                        "Storage type not yet supported.",
                    ),
                    cx,
                );
                return None;
            }
        };

        let config = if let Some(ref existing) = self.active_connection {
            StorageConfig::with_id(existing.id, name, self.selected_type, params)
        } else {
            StorageConfig::new(name, self.selected_type, params)
        };

        Some(config)
    }

    /// Get the secret (password/key) from the form.
    fn get_secret(&self, cx: &Context<Self>) -> Option<String> {
        let secret = self.secret_access_key.read(cx).value().to_string();
        if secret.is_empty() {
            None
        } else {
            Some(secret)
        }
    }

    /// Save the connection.
    fn save_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(config) = self.get_config(window, cx) {
            let secret = self.get_secret(cx);
            add_storage_connection(config, secret, cx);
            self.clear(window, cx);
            window.push_notification(
                (NotificationType::Success, "Storage connection saved."),
                cx,
            );
        }
    }

    /// Update an existing connection.
    fn update_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(config) = self.get_config(window, cx) {
            let secret = self.get_secret(cx);
            update_storage_connection(config, secret, cx);
            window.push_notification(
                (NotificationType::Success, "Storage connection updated."),
                cx,
            );
        }
    }

    /// Delete the connection.
    fn delete_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(config) = self.active_connection.clone() {
            delete_storage_connection(config, cx);
            self.clear(window, cx);
        }
    }

    /// Connect to the storage.
    fn connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(config) = self.get_config(window, cx) {
            storage_connect(&config, cx);
            self.clear(window, cx);
        }
    }

    /// Test the connection.
    fn test_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_testing {
            return;
        }

        if let Some(config) = self.get_config(window, cx) {
            self.is_testing = true;
            cx.notify();

            let entity = cx.entity();

            cx.spawn_in(window, async move |_this, cx| {
                let result = crate::state::test_storage_connection(config).await;

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

    /// Render S3-specific fields.
    fn render_s3_fields(&self, cx: &mut Context<Self>) -> impl IntoElement {
        v_form()
            .columns(2)
            .small()
            .child(
                field()
                    .col_span(2)
                    .label("Endpoint")
                    .description("Leave blank for AWS S3, set for MinIO/R2/etc.")
                    .child(Input::new(&self.endpoint)),
            )
            .child(
                field()
                    .label("Region")
                    .required(true)
                    .child(Input::new(&self.region)),
            )
            .child(
                field()
                    .label("Bucket")
                    .required(true)
                    .child(Input::new(&self.bucket)),
            )
            .child(
                field()
                    .col_span(2)
                    .label("Access Key ID")
                    .child(Input::new(&self.access_key_id)),
            )
            .child(
                field()
                    .col_span(2)
                    .label("Secret Access Key")
                    .child(Input::new(&self.secret_access_key)),
            )
            .child(
                field().col_span(2).label_indent(false).child(
                    h_flex()
                        .gap_4()
                        .child(
                            Checkbox::new("path-style")
                                .label("Path-style addressing")
                                .checked(self.path_style)
                                .on_click(cx.listener(|this, checked: &bool, _win, cx| {
                                    this.path_style = *checked;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Checkbox::new("anonymous")
                                .label("Allow anonymous access")
                                .checked(self.allow_anonymous)
                                .on_click(cx.listener(|this, checked: &bool, _win, cx| {
                                    this.allow_anonymous = *checked;
                                    cx.notify();
                                })),
                        ),
                ),
            )
    }

    /// Render local filesystem fields.
    fn render_local_fs_fields(&self) -> impl IntoElement {
        v_form().columns(1).small().child(
            field()
                .label("Root Path")
                .required(true)
                .description("Local directory to browse")
                .child(Input::new(&self.root_path)),
        )
    }
}

impl Render for StorageConnectionForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_editing = self.active_connection.is_some();
        let title = if is_editing {
            "Edit Storage Connection"
        } else {
            "Add Storage Connection"
        };

        // Update selected type from select state
        if let Some(selected) = self.storage_type.read(cx).selected_item() {
            if selected.0 != self.selected_type {
                self.selected_type = selected.0;
            }
        }

        let type_specific_fields: AnyElement = match self.selected_type {
            StorageType::S3 => self.render_s3_fields(cx).into_any_element(),
            StorageType::LocalFs => self.render_local_fs_fields().into_any_element(),
            _ => div()
                .child("This storage type is not yet supported.")
                .into_any_element(),
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
                            .label("Storage Type")
                            .child(Select::new(&self.storage_type)),
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
                                .on_click(cx.listener(|this, _, win, cx| {
                                    this.delete_connection(win, cx)
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
