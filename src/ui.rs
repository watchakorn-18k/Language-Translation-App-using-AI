use crate::google_translate::{lang_code_for_name, language_names};
use crate::providers::{load_app_preferences, save_app_preferences};
use crate::snipper::{
    OcrLanguage, capture_fullscreen_to_temp, copy_to_clipboard, crop_and_ocr,
};
use crate::snipper_overlay::open_snipper_overlay;
use crate::translator::{
    TranslatorProvider, init_config_files, retranslate_secondary, translate_text,
};
use chrono::Datelike;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::searchable_list::SearchableVec;
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::{Icon, IndexPath, StyledExt, WindowExt};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::sync::Arc;
use tokio::runtime::Runtime;

actions!(translate_app, [TriggerOcr]);

// Graphite Dark neutral ramp + single amber-ochre accent (unslop direction)
const BG_PAGE: u32 = 0x141415;
const BG_CARD: u32 = 0x19191b;
const BG_RAISED: u32 = 0x232325;
const BORDER_HAIRLINE: u32 = 0xededed14; // #ededed at ~8% opacity
const BORDER_SUBTLE: u32 = 0xededed1f; // #ededed at ~12% opacity
const TEXT_PRIMARY: u32 = 0xededed;
const TEXT_SECONDARY: u32 = 0xa1a1a6;
const TEXT_MUTED: u32 = 0x85858b;
const ACCENT_AMBER: u32 = 0xd4973b;

pub struct TranslateApp {
    runtime: Arc<Runtime>,
    input_state: Entity<InputState>,
    ocr_lang_select: Entity<SelectState<Vec<SharedString>>>,
    translator_provider: TranslatorProvider,
    secondary_lang: String,
    result_primary: String,
    result_secondary: String,
    show_results: bool,
    is_loading: bool,
    status_hint: Option<String>,
    focus_handle: FocusHandle,
}

impl TranslateApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        init_config_files();

        let runtime = Arc::new(
            Runtime::new().expect("failed to create Tokio runtime for translation requests"),
        );

        let input_state = cx.new(|cx| InputState::new(window, cx).placeholder("Text to translate"));

        let ocr_options: Vec<SharedString> = OcrLanguage::ALL
            .iter()
            .map(|l| SharedString::from(l.label()))
            .collect();
        let ocr_lang_select = cx.new(|cx| {
            SelectState::new(ocr_options, Some(IndexPath::default()), window, cx)
        });

        let focus_handle = cx.focus_handle();
        let prefs = load_app_preferences();

        Self {
            runtime,
            input_state,
            ocr_lang_select,
            translator_provider: TranslatorProvider::from_label(&prefs.translator),
            secondary_lang: prefs.target_language,
            result_primary: String::new(),
            result_secondary: String::new(),
            show_results: false,
            is_loading: false,
            status_hint: None,
            focus_handle,
        }
    }

    fn selected_ocr_language(&self, cx: &App) -> OcrLanguage {
        let label = self
            .ocr_lang_select
            .read(cx)
            .selected_value()
            .map(|s| s.to_string())
            .unwrap_or_else(|| OcrLanguage::ChineseEnglish.label().to_string());
        OcrLanguage::from_label(&label)
    }

    fn secondary_iso_code(&self) -> String {
        lang_code_for_name(&self.secondary_lang)
            .unwrap_or("th")
            .to_ascii_uppercase()
    }

    fn secondary_display_name(&self) -> String {
        let mut chars = self.secondary_lang.chars();
        match chars.next() {
            Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
            None => "Thai".to_string(),
        }
    }

    fn start_translate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_loading {
            return;
        }

        let input_text = self.input_state.read(cx).value().to_string();
        let provider = self.translator_provider;
        let secondary_lang = self.secondary_lang.clone();
        let rt = Arc::clone(&self.runtime);

        self.is_loading = true;
        self.status_hint = Some(format!("Translating with {}", provider.label()));
        cx.notify();

        cx.spawn_in(window, async move |this, cx| {
            let outcome = cx
                .background_executor()
                .spawn(async move {
                    rt.block_on(translate_text(&input_text, provider, &secondary_lang))
                })
                .await;

            let _ = this.update(cx, |app, cx| {
                app.result_primary = outcome.primary_english;
                app.result_secondary = outcome.secondary_localized;
                app.show_results = true;
                app.is_loading = false;
                app.status_hint = None;
                cx.notify();
            });
        })
        .detach();
    }

    fn start_ocr(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_loading {
            return;
        }

        let ocr_lang = self.selected_ocr_language(cx);
        let input_state = self.input_state.clone();
        let main_window = window.window_handle();

        self.is_loading = true;
        self.status_hint = Some("Drag on screen to capture region".to_string());
        cx.notify();

        cx.spawn_in(window, async move |this, cx| {
            let capture_res = cx
                .background_executor()
                .spawn(async move { capture_fullscreen_to_temp() })
                .await;

            match capture_res {
                Ok(screenshot_path) => {
                    let this_for_cb = this.clone();
                    let input_for_cb = input_state.clone();

                    let open_res = cx.update(|_window, cx| {
                        open_snipper_overlay(
                            screenshot_path,
                            Box::new(move |selection, cx| {
                                let Some((path, rect)) = selection else {
                                    // User pressed Esc / cancelled
                                    let _ = this_for_cb.update(cx, |app, cx| {
                                        app.is_loading = false;
                                        app.status_hint = None;
                                        cx.notify();
                                    });
                                    return;
                                };

                                // Update status to scanning while OCR runs
                                let _ = this_for_cb.update(cx, |app, cx| {
                                    app.show_results = true;
                                    app.status_hint = Some("OCR scanning region".to_string());
                                    app.result_primary = "OCR Scanning text".to_string();
                                    app.result_secondary = "OCR กำลังสแกนข้อความ".to_string();
                                    cx.notify();
                                });

                                let this_after = this_for_cb.clone();
                                let input_after = input_for_cb.clone();
                                cx.spawn(async move |cx| {
                                    let ocr_result = cx
                                        .background_executor()
                                        .spawn(async move { crop_and_ocr(&path, rect, ocr_lang) })
                                        .await;

                                    let _ = cx.update_window(main_window, |_, window, cx| {
                                        let _ = this_after.update(cx, |app, cx| {
                                            app.is_loading = false;
                                            app.status_hint = None;
                                            match ocr_result {
                                                Ok(scanned_text) => {
                                                    input_after.update(cx, |state, cx| {
                                                        state.set_value(scanned_text, window, cx);
                                                    });
                                                    app.result_primary =
                                                        "Scan completed, press Translate to translate"
                                                            .to_string();
                                                    app.result_secondary =
                                                        "สแกนเสร็จแล้ว กดปุ่มแปลภาษา เพื่อแปลภาษา"
                                                            .to_string();
                                                }
                                                Err(err) => {
                                                    app.result_primary = format!("OCR failed: {err}");
                                                    app.result_secondary =
                                                        "สแกน OCR ไม่สำเร็จ".to_string();
                                                }
                                            }
                                            app.show_results = true;
                                            cx.notify();
                                        });
                                    });
                                })
                                .detach();
                            }),
                            cx,
                        )
                    });

                    if let Ok(Err(err)) = open_res {
                        let _ = this.update(cx, |app, cx| {
                            app.is_loading = false;
                            app.status_hint = None;
                            app.show_results = true;
                            app.result_primary = format!("Overlay error: {err}");
                            app.result_secondary = "เปิดหน้าต่างลากครอบไม่สำเร็จ".to_string();
                            cx.notify();
                        });
                    }
                }
                Err(err) => {
                    let _ = this.update(cx, |app, cx| {
                        app.is_loading = false;
                        app.status_hint = None;
                        app.show_results = true;
                        app.result_primary = format!("Capture failed: {err}");
                        app.result_secondary = "จับภาพหน้าจอไม่สำเร็จ".to_string();
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn open_settings_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current_provider = self.translator_provider;
        let current_settings = current_provider.load_settings();

        let provider_options: Vec<SharedString> = TranslatorProvider::ALL
            .iter()
            .map(|p| SharedString::from(p.label()))
            .collect();
        let initial_idx = TranslatorProvider::ALL
            .iter()
            .position(|p| *p == current_provider)
            .unwrap_or(0);

        let select_state = cx.new(|cx| {
            SelectState::new(
                provider_options,
                Some(IndexPath::default().row(initial_idx)),
                window,
                cx,
            )
        });

        let model_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("Model ID");
            state.set_value(current_settings.model.clone(), window, cx);
            state
        });

        let api_key_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx)
                .placeholder("API key")
                .masked(true);
            state.set_value(current_settings.api_key.clone(), window, cx);
            state
        });

        let base_url_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder("Base URL");
            state.set_value(current_settings.base_url.clone(), window, cx);
            state
        });

        // Sync Model / API Key / Base URL fields whenever the user changes the Translator dropdown.
        let model_on_change = model_input.clone();
        let key_on_change = api_key_input.clone();
        let url_on_change = base_url_input.clone();
        cx.subscribe_in(
            &select_state,
            window,
            move |_this, _select, event: &SelectEvent<Vec<SharedString>>, window, cx| {
                if let SelectEvent::Confirm(Some(label)) = event {
                    let chosen = TranslatorProvider::from_label(label.as_ref());
                    let loaded = chosen.load_settings();
                    model_on_change.update(cx, |s, cx| s.set_value(loaded.model, window, cx));
                    key_on_change.update(cx, |s, cx| s.set_value(loaded.api_key, window, cx));
                    url_on_change.update(cx, |s, cx| s.set_value(loaded.base_url, window, cx));
                }
            },
        )
        .detach();

        let this_entity = cx.entity().downgrade();

        window.open_dialog(cx, move |dialog, _window, cx| {
            let active_provider = select_state
                .read(cx)
                .selected_value()
                .map(|s| TranslatorProvider::from_label(s.as_ref()))
                .unwrap_or(current_provider);

            let select_for_save = select_state.clone();
            let model_for_save = model_input.clone();
            let key_for_save = api_key_input.clone();
            let url_for_save = base_url_input.clone();
            let this_for_save = this_entity.clone();

            dialog
                .title("Settings")
                .w(px(360.))
                .centered()
                .child(
                    div()
                        .v_flex()
                        .w_full()
                        .gap(px(12.))
                        .py(px(8.))
                        // Provider row
                        .child(
                            div()
                                .v_flex()
                                .gap(px(6.))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(TEXT_SECONDARY))
                                        .child("Translator"),
                                )
                                .child(Select::new(&select_state).w_full()),
                        )
                        // Model row (for AI providers)
                        .when(active_provider.needs_model(), |form| {
                            form.child(
                                div()
                                    .v_flex()
                                    .gap(px(6.))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(TEXT_SECONDARY))
                                            .child("Model"),
                                    )
                                    .child(Input::new(&model_input).w_full())
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(TEXT_MUTED))
                                            .child(active_provider.recommended_models_hint()),
                                    ),
                            )
                        })
                        // API Key row (masked with toggle)
                        .when(active_provider.needs_api_key(), |form| {
                            form.child(
                                div()
                                    .v_flex()
                                    .gap(px(6.))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(TEXT_SECONDARY))
                                            .child("API key"),
                                    )
                                    .child(Input::new(&api_key_input).w_full().mask_toggle()),
                            )
                        })
                        // Base URL row (for Ollama / Custom OpenAI-compatible)
                        .when(active_provider.needs_base_url(), |form| {
                            form.child(
                                div()
                                    .v_flex()
                                    .gap(px(6.))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(TEXT_SECONDARY))
                                            .child("Base URL"),
                                    )
                                    .child(Input::new(&base_url_input).w_full()),
                            )
                        }),
                )
                .footer(
                    div()
                        .h_flex()
                        .items_center()
                        .justify_end()
                        .w_full()
                        .gap(px(8.))
                        .pt(px(8.))
                        .child(
                            div()
                                .id("settings-cancel-btn")
                                .h_flex()
                                .items_center()
                                .justify_center()
                                .h(px(32.))
                                .px(px(14.))
                                .rounded(px(8.))
                                .bg(rgb(BG_RAISED))
                                .border_1()
                                .border_color(rgba(BORDER_HAIRLINE))
                                .text_sm()
                                .text_color(rgb(TEXT_SECONDARY))
                                .cursor_pointer()
                                .hover(|s| s.text_color(rgb(TEXT_PRIMARY)))
                                .on_click(|_, window, cx| {
                                    window.close_dialog(cx);
                                })
                                .child("Cancel"),
                        )
                        .child(
                            div()
                                .id("settings-save-btn")
                                .h_flex()
                                .items_center()
                                .justify_center()
                                .h(px(32.))
                                .px(px(16.))
                                .rounded(px(8.))
                                .bg(rgb(TEXT_PRIMARY))
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(BG_PAGE))
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.92))
                                .on_click(move |_, window, cx| {
                                    if let Some(selected) = select_for_save.read(cx).selected_value()
                                    {
                                        let chosen =
                                            TranslatorProvider::from_label(selected.as_ref());
                                        let model_val = model_for_save.read(cx).value().to_string();
                                        let key_val = key_for_save.read(cx).value().to_string();
                                        let url_val = url_for_save.read(cx).value().to_string();
                                        chosen.save_settings(&key_val, &model_val, &url_val);

                                        if let Some(app_entity) = this_for_save.upgrade() {
                                            app_entity.update(cx, |app, cx| {
                                                app.translator_provider = chosen;
                                                save_app_preferences(
                                                    chosen.label(),
                                                    &app.secondary_lang,
                                                );
                                                cx.notify();
                                            });
                                        }
                                        window.push_notification(
                                            Notification::success("Settings saved"),
                                            cx,
                                        );
                                    }
                                    window.close_dialog(cx);
                                })
                                .child("Save"),
                        ),
                )
        });
    }

    fn open_change_language_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let langs: Vec<SharedString> = language_names()
            .into_iter()
            .map(SharedString::from)
            .collect();
        let current_lang = self.secondary_lang.clone();
        let initial_idx = langs
            .iter()
            .position(|l| l.as_ref() == current_lang)
            .unwrap_or(0);

        let select_state = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(langs),
                Some(IndexPath::default().row(initial_idx)),
                window,
                cx,
            )
            .searchable(true)
        });
        let this_entity = cx.entity().downgrade();

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let select_for_ok = select_state.clone();
            let this_for_ok = this_entity.clone();

            dialog
                .title("Target language")
                .w(px(340.))
                .centered()
                .child(
                    div()
                        .v_flex()
                        .w_full()
                        .gap(px(6.))
                        .py(px(8.))
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(TEXT_SECONDARY))
                                .child("Language"),
                        )
                        .child(Select::new(&select_state).w_full()),
                )
                .footer(
                    div()
                        .h_flex()
                        .items_center()
                        .justify_end()
                        .w_full()
                        .gap(px(8.))
                        .pt(px(8.))
                        .child(
                            div()
                                .id("lang-cancel-btn")
                                .h_flex()
                                .items_center()
                                .justify_center()
                                .h(px(32.))
                                .px(px(14.))
                                .rounded(px(8.))
                                .bg(rgb(BG_RAISED))
                                .border_1()
                                .border_color(rgba(BORDER_HAIRLINE))
                                .text_sm()
                                .text_color(rgb(TEXT_SECONDARY))
                                .cursor_pointer()
                                .hover(|s| s.text_color(rgb(TEXT_PRIMARY)))
                                .on_click(|_, window, cx| {
                                    window.close_dialog(cx);
                                })
                                .child("Cancel"),
                        )
                        .child(
                            div()
                                .id("lang-apply-btn")
                                .h_flex()
                                .items_center()
                                .justify_center()
                                .h(px(32.))
                                .px(px(16.))
                                .rounded(px(8.))
                                .bg(rgb(TEXT_PRIMARY))
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(BG_PAGE))
                                .cursor_pointer()
                                .hover(|s| s.opacity(0.92))
                                .on_click(move |_, window, cx| {
                                    if let Some(selected) = select_for_ok.read(cx).selected_value()
                                    {
                                        let new_lang = selected.to_string();
                                        if let Some(app_entity) = this_for_ok.upgrade() {
                                            app_entity.update(cx, |app, cx| {
                                                app.secondary_lang = new_lang.clone();
                                                save_app_preferences(
                                                    app.translator_provider.label(),
                                                    &app.secondary_lang,
                                                );
                                                if !app.result_primary.trim().is_empty() {
                                                    let primary = app.result_primary.clone();
                                                    let rt = Arc::clone(&app.runtime);
                                                    app.is_loading = true;
                                                    cx.notify();

                                                    cx.spawn_in(window, async move |this, cx| {
                                                        let translated = cx
                                                            .background_executor()
                                                            .spawn(async move {
                                                                rt.block_on(retranslate_secondary(
                                                                    &primary, &new_lang,
                                                                ))
                                                            })
                                                            .await;

                                                        let _ = this.update(cx, |app, cx| {
                                                            app.result_secondary = translated;
                                                            app.is_loading = false;
                                                            cx.notify();
                                                        });
                                                    })
                                                    .detach();
                                                } else {
                                                    cx.notify();
                                                }
                                            });
                                        }
                                    }
                                    window.close_dialog(cx);
                                })
                                .child("Apply"),
                        ),
                )
        });
    }
}

/// Signature detail: crisp monospace ISO language code tag.
fn iso_tag(code: impl Into<SharedString>) -> impl IntoElement {
    div()
        .h_flex()
        .items_center()
        .justify_center()
        .min_w(px(32.))
        .h(px(20.))
        .px(px(6.))
        .rounded(px(4.))
        .bg(rgb(BG_RAISED))
        .border_1()
        .border_color(rgba(BORDER_HAIRLINE))
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(TEXT_SECONDARY))
        .child(code.into())
}

/// Signature detail: tactile keyboard shortcut keycap.
fn keycap(label: &'static str) -> impl IntoElement {
    div()
        .h_flex()
        .items_center()
        .justify_center()
        .h(px(20.))
        .px(px(6.))
        .rounded(px(4.))
        .bg(rgb(BG_RAISED))
        .border_1()
        .border_color(rgba(BORDER_SUBTLE))
        .text_xs()
        .text_color(rgb(TEXT_MUTED))
        .child(label)
}

impl Render for TranslateApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current_year = chrono::Local::now().year();
        let primary_text = self.result_primary.clone();
        let secondary_text = self.result_secondary.clone();
        let provider_label = self.translator_provider.label();
        let target_iso = self.secondary_iso_code();
        let target_name = self.secondary_display_name();
        let ocr_shortcut = if cfg!(target_os = "macos") {
            "⌘⇧Z"
        } else {
            "Ctrl⇧Z"
        };

        div()
            .id("translate-app-root")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &TriggerOcr, window, cx| {
                this.start_ocr(window, cx);
            }))
            .v_flex()
            .justify_between()
            .size_full()
            .bg(rgb(BG_PAGE))
            .text_color(rgb(TEXT_PRIMARY))
            .p(px(20.))
            .gap(px(16.))
            // 1. Top Header Bar (left-aligned product title + quiet clickable engine status)
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .justify_between()
                    .w_full()
                    .h(px(32.))
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(TEXT_PRIMARY))
                            .child("Translate App with AI"),
                    )
                    .child(
                        div()
                            .id("engine-chip")
                            .h_flex()
                            .items_center()
                            .gap(px(8.))
                            .h(px(28.))
                            .px(px(10.))
                            .rounded(px(6.))
                            .bg(rgb(BG_CARD))
                            .border_1()
                            .border_color(rgba(BORDER_HAIRLINE))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(BG_RAISED)))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.open_settings_dialog(window, cx);
                            }))
                            .child(
                                div()
                                    .size(px(6.))
                                    .rounded_full()
                                    .bg(rgb(ACCENT_AMBER)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(TEXT_SECONDARY))
                                    .child(provider_label),
                            ),
                    ),
            )
            // 2. Composer Card (Input + Primary/Secondary action row)
            .child(
                div()
                    .v_flex()
                    .w_full()
                    .p(px(12.))
                    .gap(px(12.))
                    .rounded(px(12.))
                    .bg(rgb(BG_CARD))
                    .border_1()
                    .border_color(rgba(BORDER_HAIRLINE))
                    .child(Input::new(&self.input_state).w_full().cleanable(true))
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .justify_between()
                            .w_full()
                            .gap(px(8.))
                            // Secondary action: OCR Capture with keycap
                            .child(
                                div()
                                    .id("ocr-capture-action")
                                    .h_flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .h(px(36.))
                                    .px(px(12.))
                                    .rounded(px(8.))
                                    .bg(rgb(BG_RAISED))
                                    .border_1()
                                    .border_color(rgba(BORDER_HAIRLINE))
                                    .cursor_pointer()
                                    .hover(|s| s.border_color(rgba(BORDER_SUBTLE)))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.start_ocr(window, cx);
                                    }))
                                    .child(
                                        Icon::new(IconName::Crop)
                                            .size(px(16.))
                                            .text_color(rgb(TEXT_SECONDARY)),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(rgb(TEXT_PRIMARY))
                                            .child("Capture"),
                                    )
                                    .child(keycap(ocr_shortcut)),
                            )
                            // Primary action: Off-white dark-ink Translate button
                            .child(
                                div()
                                    .id("translate-primary-action")
                                    .h_flex()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(8.))
                                    .h(px(36.))
                                    .px(px(16.))
                                    .rounded(px(8.))
                                    .bg(rgb(TEXT_PRIMARY))
                                    .text_color(rgb(BG_PAGE))
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.92))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.start_translate(window, cx);
                                    }))
                                    .child(
                                        Icon::new(IconName::Languages)
                                            .size(px(16.))
                                            .text_color(rgb(BG_PAGE)),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(if self.is_loading {
                                                "Translating"
                                            } else {
                                                "Translate"
                                            }),
                                    ),
                            ),
                    )
                    // Thin loading progress only when active
                    .when(self.is_loading, |card| {
                        card.child(
                            Progress::new("translate-progress")
                                .w_full()
                                .loading(true)
                                .color(rgb(ACCENT_AMBER)),
                        )
                    }),
            )
            // 3. Moment 1: The Bilingual Translation Sheet (fills available space cleanly)
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .w_full()
                    .rounded(px(12.))
                    .bg(rgb(BG_CARD))
                    .border_1()
                    .border_color(rgba(BORDER_HAIRLINE))
                    .overflow_hidden()
                    .when(!self.show_results, |sheet| {
                        sheet.child(
                            div()
                                .v_flex()
                                .flex_1()
                                .items_start()
                                .justify_center()
                                .p(px(20.))
                                .gap(px(12.))
                                .child(
                                    div()
                                        .h_flex()
                                        .items_center()
                                        .gap(px(8.))
                                        .child(iso_tag("EN"))
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(TEXT_MUTED))
                                                .child("→"),
                                        )
                                        .child(iso_tag(target_iso.clone())),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(TEXT_SECONDARY))
                                        .child(
                                            "Enter text above or capture any region on screen to translate into English and your target language.",
                                        ),
                                )
                                .child(
                                    div()
                                        .h_flex()
                                        .items_center()
                                        .gap(px(8.))
                                        .pt(px(4.))
                                        .child(keycap(ocr_shortcut))
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(TEXT_MUTED))
                                                .child("Region OCR capture"),
                                        ),
                                ),
                        )
                    })
                    .when(self.show_results, |sheet| {
                        sheet
                            // Tier 1: Primary English Translation
                            .child(
                                div()
                                    .v_flex()
                                    .flex_1()
                                    .w_full()
                                    .p(px(16.))
                                    .gap(px(10.))
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
                                                    .gap(px(8.))
                                                    .child(iso_tag("EN"))
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(rgb(TEXT_MUTED))
                                                            .child("English"),
                                                    ),
                                            )
                                            .child(
                                                Button::new("copy-primary-btn")
                                                    .ghost()
                                                    .icon(IconName::Copy)
                                                    .tooltip("Copy")
                                                    .on_click({
                                                        let text = primary_text.clone();
                                                        move |_, window, cx| {
                                                            copy_to_clipboard(&text);
                                                            window.push_notification(
                                                                Notification::success("Copied"),
                                                                cx,
                                                            );
                                                        }
                                                    }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(TEXT_PRIMARY))
                                            .child(primary_text.clone()),
                                    ),
                            )
                            // Hairline divider between bilingual tiers
                            .child(div().w_full().h(px(1.)).bg(rgba(BORDER_HAIRLINE)))
                            // Tier 2: Secondary Localized Target Translation
                            .child(
                                div()
                                    .v_flex()
                                    .flex_1()
                                    .w_full()
                                    .p(px(16.))
                                    .gap(px(10.))
                                    .child(
                                        div()
                                            .h_flex()
                                            .items_center()
                                            .justify_between()
                                            .w_full()
                                            .child(
                                                div()
                                                    .id("change-lang-inline")
                                                    .h_flex()
                                                    .items_center()
                                                    .gap(px(8.))
                                                    .cursor_pointer()
                                                    .on_click(cx.listener(
                                                        |this, _, window, cx| {
                                                            this.open_change_language_dialog(
                                                                window, cx,
                                                            );
                                                        },
                                                    ))
                                                    .child(iso_tag(target_iso.clone()))
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(rgb(TEXT_SECONDARY))
                                                            .child(target_name.clone()),
                                                    )
                                                    .child(
                                                        Icon::new(IconName::Pencil)
                                                            .size(px(14.))
                                                            .text_color(rgb(TEXT_MUTED)),
                                                    ),
                                            )
                                            .child(
                                                Button::new("copy-secondary-btn")
                                                    .ghost()
                                                    .icon(IconName::Copy)
                                                    .tooltip("Copy")
                                                    .on_click({
                                                        let text = secondary_text.clone();
                                                        move |_, window, cx| {
                                                            copy_to_clipboard(&text);
                                                            window.push_notification(
                                                                Notification::success("Copied"),
                                                                cx,
                                                            );
                                                        }
                                                    }),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(TEXT_PRIMARY))
                                            .child(secondary_text.clone()),
                                    ),
                            )
                    }),
            )
            // 4. Moment 2: Quiet Bottom Utility Dock (OCR language + Settings + Attribution)
            .child(
                div()
                    .v_flex()
                    .w_full()
                    .gap(px(12.))
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .justify_between()
                            .w_full()
                            .gap(px(8.))
                            .child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .flex_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(TEXT_MUTED))
                                            .child("OCR"),
                                    )
                                    .child(Select::new(&self.ocr_lang_select).flex_1()),
                            )
                            .child(
                                Button::new("settings-btn")
                                    .ghost()
                                    .icon(IconName::Settings)
                                    .tooltip("Settings")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_settings_dialog(window, cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .justify_between()
                            .w_full()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(TEXT_MUTED))
                                    .child(
                                        self.status_hint
                                            .clone()
                                            .unwrap_or_else(|| format!("Target: {target_name}")),
                                    ),
                            )
                            .child(
                                div()
                                    .id("footer-link")
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(rgb(TEXT_MUTED))
                                    .hover(|s| s.text_color(rgb(TEXT_SECONDARY)))
                                    .on_click(|_, _, cx| {
                                        cx.open_url(
                                            "https://github.com/watchakorn-18k/Language-Translation-App-using-CHATGPT",
                                        );
                                    })
                                    .child(format!("wk18k © {current_year}")),
                            ),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::AssetSource;
    use gpui_kit::assets::{AllAssets, IconName};

    #[core::prelude::v1::test]
    fn all_used_icons_exist_in_all_assets() {
        let icons = [
            IconName::Crop,
            IconName::Languages,
            IconName::Pencil,
            IconName::Copy,
            IconName::Settings,
        ];
        for icon in icons {
            let path = icon.path();
            let loaded = AllAssets
                .load(path.as_ref())
                .expect("asset load should not error");
            assert!(
                loaded.is_some(),
                "Icon `{path}` missing from gpui_kit::assets::AllAssets!"
            );
        }
    }
}
