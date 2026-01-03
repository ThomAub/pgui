//! Help overlay showing keyboard shortcuts.

use gpui::*;
use gpui_component::{
    h_flex, v_flex,
    label::Label,
};

use crate::keybindings::bindings::get_all_keybindings;

/// Help overlay component showing all keyboard shortcuts.
pub struct HelpOverlay;

impl HelpOverlay {
    pub fn new() -> Self {
        Self
    }

    fn render_section(section_name: &'static str, bindings: Vec<crate::keybindings::bindings::KeybindingInfo>) -> impl IntoElement {
        v_flex()
            .gap_1()
            .mb_4()
            .child(
                Label::new(section_name)
                    .text_sm()
                    .text_color(gpui::white())
            )
            .children(bindings.into_iter().map(|kb| {
                h_flex()
                    .justify_between()
                    .gap_4()
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .bg(gpui::rgba(0x333333FF))
                            .rounded_md()
                            .child(
                                Label::new(kb.key)
                                    .text_xs()
                                    .text_color(gpui::rgba(0xFFFFFFCC))
                            )
                    )
                    .child(
                        Label::new(kb.description)
                            .text_xs()
                            .text_color(gpui::rgba(0xFFFFFFAA))
                    )
            }))
    }

    fn render_element(self) -> Stateful<Div> {
        let keybindings = get_all_keybindings();

        div()
            .id("help-overlay")
            .absolute()
            .inset_0()
            .bg(gpui::rgba(0x000000DD))
            .flex()
            .items_center()
            .justify_center()
            .child(
                v_flex()
                    .p_6()
                    .bg(gpui::rgba(0x1a1a1aFF))
                    .border_1()
                    .border_color(gpui::rgba(0x444444FF))
                    .rounded_lg()
                    .max_w(px(800.))
                    .max_h(px(600.))
                    .overflow_hidden()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .mb_4()
                            .child(
                                Label::new("Keyboard Shortcuts")
                                    .text_base()
                                    .text_color(gpui::white())
                            )
                            .child(
                                Label::new("Press Esc or ? to close")
                                    .text_xs()
                                    .text_color(gpui::rgba(0xFFFFFF66))
                            )
                    )
                    .child(
                        h_flex()
                            .gap_8()
                            .flex_wrap()
                            .children(keybindings.into_iter().map(|(name, bindings)| {
                                div()
                                    .min_w(px(200.))
                                    .child(Self::render_section(name, bindings))
                            }))
                    )
            )
    }
}

impl IntoElement for HelpOverlay {
    type Element = Stateful<Div>;

    fn into_element(self) -> Self::Element {
        self.render_element()
    }
}
