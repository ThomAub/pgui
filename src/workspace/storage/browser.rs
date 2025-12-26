//! Storage file browser component.
//!
//! Displays files and folders in the connected storage with navigation.

use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme as _, Disableable, Icon, IconName, Sizable as _, StyledExt as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    label::Label,
    list::ListItem,
    spinner::Spinner,
    v_flex,
};

use crate::{
    services::database::storage::{ObjectInfo, StorageManager},
    state::{StorageConnectionStatus, StorageState},
};

/// Events emitted by the storage browser.
pub enum StorageBrowserEvent {
    /// A file was selected.
    FileSelected(ObjectInfo),
    /// A directory was navigated to.
    DirectoryChanged(String),
}

impl EventEmitter<StorageBrowserEvent> for StorageBrowser {}

/// File browser for storage connections.
pub struct StorageBrowser {
    /// Current path in the storage.
    current_path: String,
    /// List of objects at the current path.
    objects: Vec<ObjectInfo>,
    /// Currently selected object.
    selected_object: Option<ObjectInfo>,
    /// Whether we're loading.
    is_loading: bool,
    /// Error message if loading failed.
    error: Option<String>,
    /// Storage manager reference.
    storage_manager: Option<StorageManager>,
    /// Subscriptions.
    _subscriptions: Vec<Subscription>,
}

impl StorageBrowser {
    /// Create a new storage browser view.
    pub fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::new(window, cx))
    }

    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let _subscriptions = vec![cx.observe_global_in::<StorageState>(
            window,
            move |this, _win, cx| {
                let state = cx.global::<StorageState>();

                this.storage_manager = Some(state.storage_manager.clone());
                this.current_path = state.current_path.clone();

                // Load objects if connected
                if state.connection_status == StorageConnectionStatus::Connected {
                    this.load_objects(cx);
                } else {
                    this.objects.clear();
                    this.error = None;
                }

                cx.notify();
            },
        )];

        Self {
            current_path: "/".to_string(),
            objects: vec![],
            selected_object: None,
            is_loading: false,
            error: None,
            storage_manager: None,
            _subscriptions,
        }
    }

    /// Load objects at the current path.
    fn load_objects(&mut self, cx: &mut Context<Self>) {
        let Some(manager) = self.storage_manager.clone() else {
            return;
        };

        self.is_loading = true;
        self.error = None;
        cx.notify();

        let path = self.current_path.clone();

        cx.spawn(async move |this, cx| {
            let result = manager.list(&path).await;

            this.update(cx, |this, cx| {
                this.is_loading = false;

                match result {
                    Ok(objects) => {
                        // Sort: directories first, then by name
                        let mut objects = objects;
                        objects.sort_by(|a, b| {
                            match (a.is_dir, b.is_dir) {
                                (true, false) => std::cmp::Ordering::Less,
                                (false, true) => std::cmp::Ordering::Greater,
                                _ => a.name.cmp(&b.name),
                            }
                        });
                        this.objects = objects;
                        this.error = None;
                    }
                    Err(e) => {
                        this.error = Some(e.to_string());
                        this.objects.clear();
                    }
                }

                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Navigate to a path.
    fn navigate_to(&mut self, path: String, cx: &mut Context<Self>) {
        self.current_path = path.clone();
        self.selected_object = None;

        // Update global state
        cx.update_global::<StorageState, _>(|state, _cx| {
            state.current_path = path.clone();
        });

        self.load_objects(cx);
        cx.emit(StorageBrowserEvent::DirectoryChanged(path));
    }

    /// Navigate up one level.
    fn navigate_up(&mut self, cx: &mut Context<Self>) {
        if self.current_path == "/" || self.current_path.is_empty() {
            return;
        }

        let parent = self
            .current_path
            .trim_end_matches('/')
            .rsplit_once('/')
            .map(|(parent, _)| {
                if parent.is_empty() {
                    "/".to_string()
                } else {
                    format!("{}/", parent)
                }
            })
            .unwrap_or_else(|| "/".to_string());

        self.navigate_to(parent, cx);
    }

    /// Refresh the current directory.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.load_objects(cx);
    }

    /// Select an object.
    fn select_object(&mut self, object: ObjectInfo, cx: &mut Context<Self>) {
        if object.is_dir {
            // Navigate into directory
            let new_path = if self.current_path.ends_with('/') {
                format!("{}{}", self.current_path, object.name)
            } else {
                format!("{}/{}", self.current_path, object.name)
            };
            self.navigate_to(format!("{}/", new_path), cx);
        } else {
            // Select file
            self.selected_object = Some(object.clone());
            cx.emit(StorageBrowserEvent::FileSelected(object));
            cx.notify();
        }
    }

    /// Get the icon for an object.
    fn get_object_icon(object: &ObjectInfo) -> Icon {
        if object.is_dir {
            Icon::new(IconName::Folder)
        } else if object.is_image() {
            Icon::new(IconName::Frame) // Use Frame for image files
        } else if object.is_data_file() {
            Icon::new(IconName::Frame)
        } else if object.is_previewable() {
            Icon::new(IconName::File) // Use File for text files
        } else {
            Icon::new(IconName::File)
        }
    }

    /// Render the breadcrumb navigation.
    fn render_breadcrumb(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut parts: Vec<(String, String)> = vec![("Root".to_string(), "/".to_string())];

        let path = self.current_path.trim_start_matches('/').trim_end_matches('/');
        if !path.is_empty() {
            let mut current = String::new();
            for part in path.split('/') {
                current = format!("{}/{}", current, part);
                parts.push((part.to_string(), format!("{}/", current)));
            }
        }

        h_flex()
            .gap_1()
            .items_center()
            .children(parts.iter().enumerate().map(|(i, (name, path))| {
                let path = path.clone();
                let is_last = i == parts.len() - 1;

                h_flex()
                    .gap_1()
                    .items_center()
                    .when(i > 0, |d| {
                        d.child(
                            Icon::new(IconName::ChevronRight)
                                .size_3()
                                .text_color(cx.theme().muted_foreground),
                        )
                    })
                    .child(
                        Button::new(("breadcrumb", i))
                            .ghost()
                            .small()
                            .child(name.clone())
                            .disabled(is_last)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.navigate_to(path.clone(), cx);
                            })),
                    )
            }))
    }

    /// Render the toolbar.
    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let can_go_up = self.current_path != "/" && !self.current_path.is_empty();

        h_flex()
            .gap_2()
            .items_center()
            .p_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("nav-up")
                    .icon(Icon::new(IconName::ArrowUp))
                    .ghost()
                    .small()
                    .tooltip("Go up")
                    .disabled(!can_go_up)
                    .on_click(cx.listener(|this, _, _, cx| this.navigate_up(cx))),
            )
            .child(
                Button::new("refresh")
                    .icon(Icon::empty().path("icons/rotate-ccw.svg"))
                    .ghost()
                    .small()
                    .tooltip("Refresh")
                    .disabled(self.is_loading)
                    .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
            )
            .child(div().flex_1().child(self.render_breadcrumb(cx)))
    }

    /// Render a single object item.
    fn render_object_item(
        &self,
        ix: usize,
        object: &ObjectInfo,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_selected = self
            .selected_object
            .as_ref()
            .map(|s| s.path == object.path)
            .unwrap_or(false);

        let text_color = if is_selected {
            cx.theme().accent_foreground
        } else {
            cx.theme().foreground
        };

        let bg_color = if is_selected {
            cx.theme().list_active
        } else if ix % 2 == 0 {
            cx.theme().list
        } else {
            cx.theme().list_even
        };

        let icon = Self::get_object_icon(object);
        let object_clone = object.clone();

        ListItem::new(ix)
            .w_full()
            .py_2()
            .px_3()
            .bg(bg_color)
            .border_1()
            .border_color(if is_selected {
                cx.theme().list_active_border
            } else {
                bg_color
            })
            .rounded(cx.theme().radius)
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .text_color(text_color)
                            .child(icon.size_4().text_color(text_color.opacity(0.7)))
                            .child(
                                Label::new(truncate(&object.name, 40))
                                    .font_medium()
                                    .text_sm()
                                    .whitespace_nowrap(),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_4()
                            .child(
                                Label::new(object.size_display())
                                    .text_xs()
                                    .text_color(text_color.opacity(0.6)),
                            )
                            .when(object.last_modified.is_some(), |d| {
                                d.child(
                                    Label::new(
                                        object
                                            .last_modified
                                            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                                            .unwrap_or_default(),
                                    )
                                    .text_xs()
                                    .text_color(text_color.opacity(0.6)),
                                )
                            }),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_object(object_clone.clone(), cx);
            }))
    }

    /// Render the object list.
    fn render_object_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.is_loading {
            return div()
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .child(Spinner::new())
                .into_any_element();
        }

        if let Some(ref error) = self.error {
            return div()
                .flex()
                .flex_1()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_2()
                .child(
                    Icon::new(IconName::TriangleAlert)
                        .size_8()
                        .text_color(cx.theme().danger),
                )
                .child(Label::new("Failed to load").font_semibold())
                .child(
                    Label::new(error.clone())
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                )
                .into_any_element();
        }

        if self.objects.is_empty() {
            return div()
                .flex()
                .flex_1()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_2()
                .child(
                    Icon::new(IconName::FolderOpen)
                        .size_8()
                        .text_color(cx.theme().muted_foreground),
                )
                .child(Label::new("Empty directory").text_color(cx.theme().muted_foreground))
                .into_any_element();
        }

        let mut container = div()
            .flex()
            .flex_col()
            .flex_1()
            .overflow_hidden()
            .p_2()
            .gap_1();

        for (ix, obj) in self.objects.iter().enumerate() {
            container = container.child(self.render_object_item(ix, obj, cx));
        }

        container.into_any_element()
    }

    /// Render the details panel for the selected object.
    fn render_details_panel(&self, cx: &Context<Self>) -> impl IntoElement {
        let Some(ref object) = self.selected_object else {
            return div()
                .w(px(250.))
                .p_4()
                .border_l_1()
                .border_color(cx.theme().border)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Label::new("Select a file to view details")
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                )
                .into_any_element();
        };

        let icon = Self::get_object_icon(object);

        div()
            .w(px(250.))
            .p_4()
            .border_l_1()
            .border_color(cx.theme().border)
            .flex()
            .flex_col()
            .gap_4()
            .child(
                v_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        icon
                            .size_12()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new(object.name.clone())
                            .font_semibold()
                            .text_center(),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .justify_between()
                            .child(Label::new("Size").text_sm().text_color(cx.theme().muted_foreground))
                            .child(Label::new(object.size_display()).text_sm()),
                    )
                    .when(object.last_modified.is_some(), |d| {
                        d.child(
                            h_flex()
                                .justify_between()
                                .child(
                                    Label::new("Modified")
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground),
                                )
                                .child(Label::new(
                                    object
                                        .last_modified
                                        .map(|dt| dt.format("%Y-%m-%d").to_string())
                                        .unwrap_or_default(),
                                ).text_sm()),
                        )
                    })
                    .when(object.content_type.is_some(), |d| {
                        d.child(
                            h_flex()
                                .justify_between()
                                .child(
                                    Label::new("Type")
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground),
                                )
                                .child(
                                    Label::new(object.content_type.clone().unwrap_or_default())
                                        .text_sm(),
                                ),
                        )
                    })
                    .child(
                        h_flex()
                            .justify_between()
                            .child(
                                Label::new("Path")
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                Label::new(truncate(&object.path, 20))
                                    .text_sm()
                                    .text_color(cx.theme().foreground.opacity(0.8)),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        Button::new("download")
                            .child("Download")
                            .w_full()
                            .small(),
                    )
                    .when(object.is_previewable(), |d| {
                        d.child(
                            Button::new("preview")
                                .child("Preview")
                                .ghost()
                                .w_full()
                                .small(),
                        )
                    }),
            )
            .into_any_element()
    }
}

impl Render for StorageBrowser {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .flex_1()
            .h_full()
            .child(self.render_toolbar(cx))
            .child(
                h_flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .child(self.render_object_list(cx)),
                    )
                    .child(self.render_details_panel(cx)),
            )
    }
}

/// Truncate a string to a maximum length.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}
