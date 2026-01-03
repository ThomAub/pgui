//! Storage connection list components.

use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, IndexPath, Selectable, StyledExt, h_flex, label::Label,
    list::{ListDelegate, ListItem, ListState},
    v_flex,
};

use crate::services::database::storage::{StorageConfig, StorageType};

/// List item for a storage connection.
#[derive(IntoElement)]
pub struct StorageConnectionListItem {
    base: ListItem,
    ix: IndexPath,
    connection: StorageConfig,
    selected: bool,
}

impl StorageConnectionListItem {
    pub fn new(
        id: impl Into<ElementId>,
        connection: StorageConfig,
        ix: IndexPath,
        selected: bool,
    ) -> Self {
        Self {
            connection,
            ix,
            base: ListItem::new(id),
            selected,
        }
    }

    fn get_icon(&self) -> Icon {
        match self.connection.storage_type {
            StorageType::S3 => Icon::empty().path("icons/cloud-download.svg"),
            StorageType::Gcs => Icon::empty().path("icons/cloud-download.svg"),
            StorageType::AzureBlob => Icon::empty().path("icons/cloud-download.svg"),
            StorageType::LocalFs => Icon::new(IconName::Folder),
        }
    }
}

impl Selectable for StorageConnectionListItem {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl RenderOnce for StorageConnectionListItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let text_color = if self.selected {
            cx.theme().accent_foreground
        } else {
            cx.theme().foreground
        };

        let bg_color = if self.selected {
            cx.theme().list_active
        } else if self.ix.row % 2 == 0 {
            cx.theme().list
        } else {
            cx.theme().list_even
        };

        let description = match &self.connection.params {
            crate::services::database::storage::StorageParams::S3 { bucket, region, .. } => {
                format!("{} ({})", bucket, region)
            }
            crate::services::database::storage::StorageParams::LocalFs { root_path } => {
                root_path.display().to_string()
            }
            crate::services::database::storage::StorageParams::Gcs { bucket, .. } => {
                bucket.clone()
            }
            crate::services::database::storage::StorageParams::AzureBlob { container, .. } => {
                container.clone()
            }
        };

        let icon = self.get_icon();

        self.base
            .px_3()
            .py_2()
            .overflow_x_hidden()
            .bg(bg_color)
            .border_1()
            .border_color(bg_color)
            .when(self.selected, |this| {
                this.border_color(cx.theme().list_active_border)
            })
            .rounded(cx.theme().radius)
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .text_color(text_color)
                    .child(
                        icon
                            .size_4()
                            .text_color(text_color.opacity(0.7)),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .flex_1()
                            .overflow_x_hidden()
                            .child(
                                Label::new(self.connection.name.clone())
                                    .font_semibold()
                                    .whitespace_nowrap(),
                            )
                            .child(
                                Label::new(description)
                                    .text_xs()
                                    .text_color(text_color.opacity(0.6))
                                    .whitespace_nowrap(),
                            ),
                    ),
            )
    }
}

/// Delegate for the storage connection list.
pub struct StorageConnectionListDelegate {
    connections: Vec<StorageConfig>,
    pub matched_connections: Vec<StorageConfig>,
    selected_index: Option<IndexPath>,
}

impl ListDelegate for StorageConnectionListDelegate {
    type Item = StorageConnectionListItem;

    fn items_count(&self, _section: usize, _app: &App) -> usize {
        self.matched_connections.len()
    }

    fn confirm(
        &mut self,
        _secondary: bool,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) {
        if let Some(selected) = self.selected_index {
            if let Some(conn) = self.matched_connections.get(selected.row) {
                tracing::debug!("Selected storage connection: {}", conn.name);
            }
        }
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = ix;
        cx.notify();
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let selected = Some(ix) == self.selected_index;
        if let Some(conn) = self.matched_connections.get(ix.row) {
            return Some(StorageConnectionListItem::new(
                ix,
                conn.clone(),
                ix,
                selected,
            ));
        }
        None
    }
}

impl StorageConnectionListDelegate {
    pub fn new() -> Self {
        Self {
            connections: vec![],
            matched_connections: vec![],
            selected_index: None,
        }
    }

    pub fn update_connections(&mut self, connections: Vec<StorageConfig>) {
        self.connections = connections;
        self.matched_connections = self.connections.clone();
        if !self.matched_connections.is_empty() && self.selected_index.is_none() {
            self.selected_index = Some(IndexPath::default());
        }
    }
}
