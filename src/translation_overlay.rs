use crate::snipper::copy_to_clipboard;
use crate::theme::Palette;
use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, StyledExt};
use gpui_kit::*;

actions!(translation_overlay, [CloseTranslationOverlay]);

/// Floating translation HUD popup displayed near the captured screen region
/// (designed for reading games, manga, or full-screen PDFs without switching windows).
pub struct TranslationOverlay {
    primary_english: String,
    secondary_localized: String,
    target_iso: String,
    palette: Palette,
    focus_handle: FocusHandle,
}

impl TranslationOverlay {
    pub fn new(
        primary_english: String,
        secondary_localized: String,
        target_iso: String,
        palette: Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        Self {
            primary_english,
            secondary_localized,
            target_iso,
            palette,
            focus_handle,
        }
    }
}

impl Render for TranslationOverlay {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette;
        let en = self.primary_english.clone();
        let target = self.secondary_localized.clone();
        let iso = self.target_iso.clone();

        div()
            .id("translation-hud-overlay")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|_, _: &CloseTranslationOverlay, window, _| {
                window.remove_window();
            }))
            .v_flex()
            .size_full()
            .p(px(14.))
            .gap(px(10.))
            .rounded(px(12.))
            .bg(rgb(p.card))
            .border_1()
            .border_color(rgba(p.border_subtle))
            .text_color(rgb(p.text_primary))
            // Header
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .w_full()
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap(px(6.))
                            .child(div().size(px(6.)).rounded_full().bg(rgb(p.accent)))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(p.text_secondary))
                                    .child(format!("EN → {iso}")),
                            ),
                    )
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                div()
                                    .id("hud-copy-btn")
                                    .p(px(4.))
                                    .rounded(px(6.))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(p.raised)))
                                    .on_click({
                                        let text = target.clone();
                                        move |_, _, _| copy_to_clipboard(&text)
                                    })
                                    .child(
                                        Icon::new(IconName::Copy)
                                            .size(px(14.))
                                            .text_color(rgb(p.text_muted)),
                                    ),
                            )
                            .child(
                                div()
                                    .id("hud-close-btn")
                                    .p(px(4.))
                                    .rounded(px(6.))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(p.raised)))
                                    .on_click(|_, window, _| window.remove_window())
                                    .child(
                                        Icon::new(IconName::X)
                                            .size(px(14.))
                                            .text_color(rgb(p.text_muted)),
                                    ),
                            ),
                    ),
            )
            // Localized target translation (most prominent)
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(p.text_primary))
                    .child(target),
            )
            .child(div().w_full().h(px(1.)).bg(rgba(p.border_hairline)))
            // English bridge translation
            .child(div().text_xs().text_color(rgb(p.text_secondary)).child(en))
    }
}

/// Opens a compact floating translation HUD window near the top-center of the display.
pub fn open_translation_overlay(
    primary_english: String,
    secondary_localized: String,
    target_iso: String,
    palette: Palette,
    cx: &mut App,
) {
    let bounds = Bounds::centered(None, size(px(380.), px(164.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        focus: true,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: true,
        ..Default::default()
    };

    let _ = cx.open_window(options, |window, cx| {
        cx.new(|cx| {
            TranslationOverlay::new(
                primary_english,
                secondary_localized,
                target_iso,
                palette,
                window,
                cx,
            )
        })
    });
}
