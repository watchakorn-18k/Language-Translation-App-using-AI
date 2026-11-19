use crate::clipboard_watch::ClipboardWatcher;
use crate::glossary::GlossaryStore;
use crate::google_translate::{lang_code_for_name, language_names};
use crate::history::HistoryStore;
use crate::hotkey::GlobalOcrHotkey;
use crate::paths;
use crate::providers::{AppPreferences, load_app_preferences, save_app_preferences};
use crate::snipper::{
    OcrLanguage, capture_display_to_temp, copy_to_clipboard, crop_and_ocr, crop_selection_to_file,
    read_from_clipboard,
};
use crate::snipper_overlay::open_snipper_overlay;
use crate::theme::{Palette, ThemeChoice};
use crate::translation_overlay::open_translation_overlay;
use crate::translator::{
    TranslateOptions, TranslatorProvider, init_config_files, retranslate_secondary,
    translate_text_with_options,
};
use crate::tts::speak_text;
use crate::updater::{UpdateInfo, check_for_update, download_release_asset};
use crate::vision_ocr::vision_ocr_and_translate;
use crate::window_level::set_always_on_top;
use chrono::Datelike;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::searchable_list::SearchableVec;
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Icon, IndexPath, StyledExt, Theme, ThemeMode, WindowExt};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

actions!(translate_app, [TriggerOcr]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    Engine,
    Behavior,
}

pub struct TranslateApp {
    runtime: Arc<Runtime>,
    input_state: Entity<InputState>,
    ocr_lang_select: Entity<SelectState<Vec<SharedString>>>,
    translator_provider: TranslatorProvider,
    prefs: AppPreferences,
    settings_tab: SettingsTab,
    history: HistoryStore,
    glossary: GlossaryStore,
    clipboard_watcher: ClipboardWatcher,
    update_info: Option<UpdateInfo>,
    _global_hotkey: Option<GlobalOcrHotkey>,
    result_primary: String,
    result_secondary: String,
    show_results: bool,
    is_loading: bool,
    status_hint: Option<String>,
    focus_handle: FocusHandle,
}

impl TranslateApp {
    pub fn new(
        window: &mut Window,
        global_hotkey: Option<GlobalOcrHotkey>,
        cx: &mut Context<Self>,
    ) -> Self {
        init_config_files();

        let runtime = Arc::new(
            Runtime::new().expect("failed to create Tokio runtime for translation requests"),
        );

        let input_state = cx.new(|cx| InputState::new(window, cx).placeholder("Text to translate"));

        let prefs = load_app_preferences();
        let ocr_options: Vec<SharedString> = OcrLanguage::ALL
            .iter()
            .map(|l| SharedString::from(l.label()))
            .collect();
        let initial_ocr_idx = OcrLanguage::ALL
            .iter()
            .position(|l| l.label() == prefs.ocr_language)
            .unwrap_or(2);
        let ocr_lang_select = cx.new(|cx| {
            SelectState::new(
                ocr_options,
                Some(IndexPath::default().row(initial_ocr_idx)),
                window,
                cx,
            )
        });

        // Persist OCR language selection when changed in the bottom dock
        cx.subscribe_in(
            &ocr_lang_select,
            window,
            move |this, _select, event: &SelectEvent<Vec<SharedString>>, _window, cx| {
                if let SelectEvent::Confirm(Some(label)) = event {
                    this.prefs.ocr_language = label.to_string();
                    this.persist_prefs();
                    cx.notify();
                }
            },
        )
        .detach();

        let focus_handle = cx.focus_handle();
        let translator_provider = TranslatorProvider::from_label(&prefs.translator);
        let history = HistoryStore::default();
        let glossary = GlossaryStore::default();
        let clipboard_watcher = ClipboardWatcher::new(read_from_clipboard());

        // Phase 1 & 4: Poll OS-wide global hotkey registered on main thread
        if global_hotkey.is_some() {
            cx.spawn_in(window, async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(60))
                        .await;
                    let triggered = this
                        .read_with(cx, |app, _| {
                            app._global_hotkey
                                .as_ref()
                                .map(|hk| hk.poll_triggered())
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    if triggered {
                        let _ = this.update_in(cx, |app, window, cx| {
                            app.start_ocr(window, cx);
                        });
                    }
                }
            })
            .detach();
        }

        // Phase 6: Clipboard watcher loop (active when `prefs.clipboard_watch` is enabled)
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(450))
                    .await;
                let enabled = this
                    .read_with(cx, |app, _| app.prefs.clipboard_watch && !app.is_loading)
                    .unwrap_or(false);
                if !enabled {
                    continue;
                }
                let clip = cx
                    .background_executor()
                    .spawn(async move { read_from_clipboard() })
                    .await;
                let _ = this.update_in(cx, |app, window, cx| {
                    if let Some(new_text) = app.clipboard_watcher.poll(&clip) {
                        app.input_state.update(cx, |s, cx| {
                            s.set_value(new_text, window, cx);
                        });
                        app.start_translate(window, cx);
                    }
                });
            }
        })
        .detach();

        // Phase 3 & 5: Check GitHub Releases asynchronously for app updates
        let rt_for_update = Arc::clone(&runtime);
        cx.spawn_in(window, async move |this, cx| {
            let current = env!("CARGO_PKG_VERSION").to_string();
            let found = cx
                .background_executor()
                .spawn(async move { rt_for_update.block_on(check_for_update(&current)) })
                .await;
            if let Some(info) = found {
                let _ = this.update(cx, |app, cx| {
                    app.update_info = Some(info);
                    cx.notify();
                });
            }
        })
        .detach();

        Self {
            runtime,
            input_state,
            ocr_lang_select,
            translator_provider,
            prefs,
            settings_tab: SettingsTab::Engine,
            history,
            glossary,
            clipboard_watcher,
            update_info: None,
            _global_hotkey: global_hotkey,
            result_primary: String::new(),
            result_secondary: String::new(),
            show_results: false,
            is_loading: false,
            status_hint: None,
            focus_handle,
        }
    }

    fn palette(&self, window: &Window) -> Palette {
        let system_dark = matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        Palette::for_choice(self.prefs.theme, system_dark)
    }

    fn apply_theme(&self, window: &mut Window, cx: &mut App) {
        let system_dark = matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        let mode = if self.prefs.theme.is_dark(system_dark) {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        };
        Theme::change(mode, Some(window), cx);
    }

    fn persist_prefs(&self) {
        save_app_preferences(&self.prefs);
    }

    fn selected_ocr_language(&self, cx: &App) -> OcrLanguage {
        let label = self
            .ocr_lang_select
            .read(cx)
            .selected_value()
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.prefs.ocr_language.clone());
        OcrLanguage::from_label(&label)
    }

    fn secondary_iso_code(&self) -> String {
        lang_code_for_name(&self.prefs.target_language)
            .unwrap_or("th")
            .to_ascii_uppercase()
    }

    fn secondary_display_name(&self) -> String {
        let mut chars = self.prefs.target_language.chars();
        match chars.next() {
            Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
            None => "Thai".to_string(),
        }
    }

    fn start_translate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.start_translate_inner(false, window, cx);
    }

    fn start_translate_inner(
        &mut self,
        from_ocr: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_loading {
            return;
        }

        let input_text = self.input_state.read(cx).value().to_string();
        let provider = self.translator_provider;
        let secondary_lang = self.prefs.target_language.clone();
        let show_overlay = from_ocr && self.prefs.translation_overlay;
        let palette = self.palette(window);
        let target_iso = self.secondary_iso_code();
        let options = TranslateOptions {
            direct_target_translation: self.prefs.direct_target_translation,
            streaming_enabled: self.prefs.streaming_enabled,
        };
        let rt = Arc::clone(&self.runtime);

        self.is_loading = true;
        self.status_hint = Some(format!("Translating with {}", provider.label()));
        if options.streaming_enabled && provider != TranslatorProvider::GoogleTranslate {
            self.show_results = true;
            self.result_primary.clear();
            self.result_secondary = "Streaming response...".to_string();
        }
        cx.notify();

        let (tx, mut rx) = mpsc::unbounded_channel::<String>();

        if options.streaming_enabled && provider != TranslatorProvider::GoogleTranslate {
            cx.spawn_in(window, async move |this, cx| {
                let mut streamed = String::new();
                while let Some(chunk) = rx.recv().await {
                    streamed.push_str(&chunk);
                    let snapshot = streamed.clone();
                    let _ = this.update(cx, |app, cx| {
                        if app.is_loading {
                            app.show_results = true;
                            app.result_primary = snapshot;
                            cx.notify();
                        }
                    });
                }
            })
            .detach();
        }

        let source_for_history = input_text.clone();
        cx.spawn_in(window, async move |this, cx| {
            let outcome = cx
                .background_executor()
                .spawn(async move {
                    rt.block_on(translate_text_with_options(
                        &input_text,
                        provider,
                        &secondary_lang,
                        options,
                        Some(tx),
                    ))
                })
                .await;

            let _ = this.update(cx, |app, cx| {
                app.result_primary = outcome.primary_english.clone();
                app.result_secondary = outcome.secondary_localized.clone();
                app.show_results = true;
                app.is_loading = false;
                app.status_hint = None;
                if !source_for_history.trim().is_empty() {
                    app.history.push(
                        provider.label(),
                        &source_for_history,
                        &outcome.primary_english,
                        &app.prefs.target_language,
                        &outcome.secondary_localized,
                    );
                }
                if show_overlay {
                    open_translation_overlay(
                        outcome.primary_english,
                        outcome.secondary_localized,
                        target_iso,
                        palette,
                        cx,
                    );
                }
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
        let use_vision = self.prefs.vision_ocr && self.translator_provider.supports_vision();
        let auto_translate = self.prefs.auto_translate_after_ocr;
        let show_overlay = self.prefs.translation_overlay;
        let palette = self.palette(window);
        let target_iso = self.secondary_iso_code();
        let provider = self.translator_provider;
        let target_lang = self.prefs.target_language.clone();
        let rt = Arc::clone(&self.runtime);

        let active_display = window.display(cx);
        let target_bounds = active_display.as_ref().map(|d| d.bounds());
        let display_index = active_display.as_ref().and_then(|active| {
            cx.displays()
                .iter()
                .position(|d| d.id() == active.id())
                .map(|i| i + 1)
        });

        self.is_loading = true;
        self.status_hint = Some("Drag on screen to capture region".to_string());
        cx.notify();

        cx.spawn_in(window, async move |this, cx| {
            let capture_res = cx
                .background_executor()
                .spawn(async move { capture_display_to_temp(display_index) })
                .await;

            match capture_res {
                Ok(screenshot_path) => {
                    let this_for_cb = this.clone();
                    let input_for_cb = input_state.clone();

                    let open_res = cx.update(|_window, cx| {
                        open_snipper_overlay(
                            screenshot_path,
                            target_bounds,
                            Box::new(move |selection, cx| {
                                let Some((path, rect)) = selection else {
                                    let _ = this_for_cb.update(cx, |app, cx| {
                                        app.is_loading = false;
                                        app.status_hint = None;
                                        cx.notify();
                                    });
                                    return;
                                };

                                let _ = this_for_cb.update(cx, |app, cx| {
                                    app.show_results = true;
                                    app.status_hint = Some(if use_vision {
                                        format!("Vision OCR with {}", provider.label())
                                    } else {
                                        "OCR scanning region".to_string()
                                    });
                                    app.result_primary = "OCR Scanning text".to_string();
                                    app.result_secondary = "OCR กำลังสแกนข้อความ".to_string();
                                    cx.notify();
                                });

                                let this_after = this_for_cb.clone();
                                let input_after = input_for_cb.clone();
                                let rt_inner = Arc::clone(&rt);
                                let target_inner = target_lang.clone();
                                let iso_inner = target_iso.clone();

                                cx.spawn(async move |cx| {
                                    if use_vision {
                                        let vision_res = cx
                                            .background_executor()
                                            .spawn(async move {
                                                let cropped = crop_selection_to_file(&path, rect)
                                                    .map_err(|e| e.to_string())?;
                                                rt_inner
                                                    .block_on(vision_ocr_and_translate(
                                                        &cropped,
                                                        provider,
                                                        &target_inner,
                                                    ))
                                                    .map_err(|e| e.to_string())
                                            })
                                            .await;

                                        let _ = cx.update_window(main_window, |_, window, cx| {
                                            let _ = this_after.update(cx, |app, cx| {
                                                app.is_loading = false;
                                                app.status_hint = None;
                                                match vision_res {
                                                    Ok(out) => {
                                                        app.clipboard_watcher
                                                            .mark_self_copy(&out.source_text);
                                                        copy_to_clipboard(&out.source_text);
                                                        input_after.update(cx, |state, cx| {
                                                            state.set_value(
                                                                out.source_text.clone(),
                                                                window,
                                                                cx,
                                                            );
                                                        });
                                                        app.result_primary =
                                                            out.primary_english.clone();
                                                        app.result_secondary =
                                                            out.secondary_localized.clone();
                                                        app.history.push(
                                                            provider.label(),
                                                            &out.source_text,
                                                            &out.primary_english,
                                                            &app.prefs.target_language,
                                                            &out.secondary_localized,
                                                        );
                                                        if show_overlay {
                                                            open_translation_overlay(
                                                                out.primary_english,
                                                                out.secondary_localized,
                                                                iso_inner,
                                                                palette,
                                                                cx,
                                                            );
                                                        }
                                                    }
                                                    Err(err) => {
                                                        app.result_primary =
                                                            format!("Vision OCR failed: {err}");
                                                        app.result_secondary =
                                                            "Vision OCR ไม่สำเร็จ".to_string();
                                                    }
                                                }
                                                app.show_results = true;
                                                cx.notify();
                                            });
                                        });
                                    } else {
                                        let ocr_result = cx
                                            .background_executor()
                                            .spawn(
                                                async move { crop_and_ocr(&path, rect, ocr_lang) },
                                            )
                                            .await;

                                        let _ = cx.update_window(main_window, |_, window, cx| {
                                            let _ = this_after.update(cx, |app, cx| {
                                                app.is_loading = false;
                                                app.status_hint = None;
                                                match ocr_result {
                                                    Ok(scanned_text) => {
                                                        app.clipboard_watcher
                                                            .mark_self_copy(&scanned_text);
                                                        let has_text =
                                                            !scanned_text.trim().is_empty();
                                                        input_after.update(cx, |state, cx| {
                                                            state.set_value(
                                                                scanned_text,
                                                                window,
                                                                cx,
                                                            );
                                                        });
                                                        if auto_translate && has_text {
                                                            app.start_translate_inner(
                                                                true, window, cx,
                                                            );
                                                            return;
                                                        }
                                                        app.result_primary =
                                                            "Scan completed, press Translate"
                                                                .to_string();
                                                        app.result_secondary =
                                                            "สแกนเสร็จแล้ว กดปุ่มแปลภาษา".to_string();
                                                    }
                                                    Err(err) => {
                                                        app.result_primary =
                                                            format!("OCR failed: {err}");
                                                        app.result_secondary =
                                                            "สแกน OCR ไม่สำเร็จ".to_string();
                                                    }
                                                }
                                                app.show_results = true;
                                                cx.notify();
                                            });
                                        });
                                    }
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

    fn download_update(&mut self, info: UpdateInfo, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(url), Some(asset_name)) = (info.download_url.clone(), info.asset_name.clone())
        else {
            cx.open_url(&info.release_url);
            return;
        };

        let rt = Arc::clone(&self.runtime);
        let dest_dir = paths::downloads_dir();
        let fallback_url = info.release_url;
        self.status_hint = Some(format!("Downloading {asset_name}..."));
        cx.notify();

        cx.spawn_in(window, async move |this, cx| {
            let res =
                cx.background_executor()
                    .spawn(async move {
                        rt.block_on(download_release_asset(&url, &asset_name, &dest_dir))
                    })
                    .await;

            let _ = this.update_in(cx, |app, window, cx| {
                app.status_hint = None;
                match res {
                    Ok(saved_path) => {
                        cx.reveal_path(&saved_path);
                        window.push_notification(
                            Notification::success(format!(
                                "Downloaded to {}",
                                saved_path.display()
                            )),
                            cx,
                        );
                    }
                    Err(_) => {
                        cx.open_url(&fallback_url);
                    }
                }
                cx.notify();
            });
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

        let theme_options: Vec<SharedString> = ThemeChoice::ALL
            .iter()
            .map(|t| SharedString::from(t.label()))
            .collect();
        let initial_theme_idx = ThemeChoice::ALL
            .iter()
            .position(|t| *t == self.prefs.theme)
            .unwrap_or(0);
        let theme_select = cx.new(|cx| {
            SelectState::new(
                theme_options,
                Some(IndexPath::default().row(initial_theme_idx)),
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

        let this_theme = cx.entity().downgrade();
        cx.subscribe_in(
            &theme_select,
            window,
            move |_this, _select, event: &SelectEvent<Vec<SharedString>>, window, cx| {
                if let SelectEvent::Confirm(Some(label)) = event {
                    let chosen = ThemeChoice::from_label(label.as_ref());
                    if let Some(app_entity) = this_theme.upgrade() {
                        app_entity.update(cx, |app, cx| {
                            app.prefs.theme = chosen;
                            app.persist_prefs();
                            app.apply_theme(window, cx);
                            cx.notify();
                        });
                    }
                }
            },
        )
        .detach();

        let this_entity = cx.entity().downgrade();

        window.open_dialog(cx, move |dialog, window, cx| {
            let active_provider = select_state
                .read(cx)
                .selected_value()
                .map(|s| TranslatorProvider::from_label(s.as_ref()))
                .unwrap_or(current_provider);

            let (current_prefs, active_tab, palette) = this_entity
                .upgrade()
                .map(|e| {
                    let app = e.read(cx);
                    (app.prefs.clone(), app.settings_tab, app.palette(window))
                })
                .unwrap_or((
                    AppPreferences::default(),
                    SettingsTab::Engine,
                    Palette::DARK,
                ));

            let select_for_save = select_state.clone();
            let model_for_save = model_input.clone();
            let key_for_save = api_key_input.clone();
            let url_for_save = base_url_input.clone();
            let this_for_save = this_entity.clone();

            let this_tab_engine = this_entity.clone();
            let this_tab_behavior = this_entity.clone();
            let this_auto = this_entity.clone();
            let this_direct = this_entity.clone();
            let this_vision = this_entity.clone();
            let this_stream = this_entity.clone();
            let this_overlay = this_entity.clone();
            let this_clipboard = this_entity.clone();
            let this_keychain = this_entity.clone();

            let tab_bar = div()
                .h_flex()
                .items_center()
                .w_full()
                .p(px(4.))
                .gap(px(4.))
                .rounded(px(8.))
                .bg(rgb(palette.raised))
                .border_1()
                .border_color(rgba(palette.border_hairline))
                .child(
                    div()
                        .id("tab-engine")
                        .flex_1()
                        .h_flex()
                        .items_center()
                        .justify_center()
                        .h(px(28.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .when(active_tab == SettingsTab::Engine, |d| {
                            d.bg(rgb(palette.card))
                                .text_color(rgb(palette.text_primary))
                        })
                        .when(active_tab != SettingsTab::Engine, |d| {
                            d.text_color(rgb(palette.text_muted))
                        })
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .on_click(move |_, _, cx| {
                            if let Some(e) = this_tab_engine.upgrade() {
                                e.update(cx, |app, cx| {
                                    app.settings_tab = SettingsTab::Engine;
                                    cx.notify();
                                });
                            }
                        })
                        .child("Engine"),
                )
                .child(
                    div()
                        .id("tab-behavior")
                        .flex_1()
                        .h_flex()
                        .items_center()
                        .justify_center()
                        .h(px(28.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .when(active_tab == SettingsTab::Behavior, |d| {
                            d.bg(rgb(palette.card))
                                .text_color(rgb(palette.text_primary))
                        })
                        .when(active_tab != SettingsTab::Behavior, |d| {
                            d.text_color(rgb(palette.text_muted))
                        })
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .on_click(move |_, _, cx| {
                            if let Some(e) = this_tab_behavior.upgrade() {
                                e.update(cx, |app, cx| {
                                    app.settings_tab = SettingsTab::Behavior;
                                    cx.notify();
                                });
                            }
                        })
                        .child("Behavior"),
                );

            dialog
                .title("Settings")
                .w(px(372.))
                .centered()
                .child(
                    div()
                        .v_flex()
                        .w_full()
                        .gap(px(12.))
                        .py(px(6.))
                        .child(tab_bar)
                        .when(active_tab == SettingsTab::Engine, |body| {
                            body.child(
                                div()
                                    .v_flex()
                                    .gap(px(6.))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(palette.text_secondary))
                                            .child("Translator"),
                                    )
                                    .child(Select::new(&select_state).w_full()),
                            )
                            .when(active_provider.needs_model(), |form| {
                                form.child(
                                    div()
                                        .v_flex()
                                        .gap(px(6.))
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(palette.text_secondary))
                                                .child("Model"),
                                        )
                                        .child(Input::new(&model_input).w_full())
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(palette.text_muted))
                                                .child(active_provider.recommended_models_hint()),
                                        ),
                                )
                            })
                            .when(active_provider.needs_api_key(), |form| {
                                form.child(
                                    div()
                                        .v_flex()
                                        .gap(px(6.))
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(palette.text_secondary))
                                                .child("API key"),
                                        )
                                        .child(Input::new(&api_key_input).w_full().mask_toggle()),
                                )
                            })
                            .when(
                                active_provider.needs_base_url(),
                                |form| {
                                    form.child(
                                        div()
                                            .v_flex()
                                            .gap(px(6.))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(rgb(palette.text_secondary))
                                                    .child("Base URL"),
                                            )
                                            .child(Input::new(&base_url_input).w_full()),
                                    )
                                },
                            )
                        })
                        .when(active_tab == SettingsTab::Behavior, |body| {
                            body.child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .justify_between()
                                    .w_full()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(palette.text_secondary))
                                            .child("Appearance"),
                                    )
                                    .child(Select::new(&theme_select).w(px(140.))),
                            )
                            .child(div().w_full().h(px(1.)).bg(rgba(palette.border_hairline)))
                            .child(
                                div()
                                    .v_flex()
                                    .gap(px(10.))
                                    .child(setting_toggle_row(
                                        "sw-auto-ocr",
                                        "Auto-translate after OCR",
                                        current_prefs.auto_translate_after_ocr,
                                        palette,
                                        move |checked, _, cx| {
                                            if let Some(e) = this_auto.upgrade() {
                                                e.update(cx, |app, cx| {
                                                    app.prefs.auto_translate_after_ocr = *checked;
                                                    app.persist_prefs();
                                                    cx.notify();
                                                });
                                            }
                                        },
                                    ))
                                    .child(setting_toggle_row(
                                        "sw-direct-ai",
                                        "Direct AI translation (single-hop)",
                                        current_prefs.direct_target_translation,
                                        palette,
                                        move |checked, _, cx| {
                                            if let Some(e) = this_direct.upgrade() {
                                                e.update(cx, |app, cx| {
                                                    app.prefs.direct_target_translation = *checked;
                                                    app.persist_prefs();
                                                    cx.notify();
                                                });
                                            }
                                        },
                                    ))
                                    .child(setting_toggle_row(
                                        "sw-vision-ocr",
                                        "Vision AI OCR (image to text)",
                                        current_prefs.vision_ocr,
                                        palette,
                                        move |checked, _, cx| {
                                            if let Some(e) = this_vision.upgrade() {
                                                e.update(cx, |app, cx| {
                                                    app.prefs.vision_ocr = *checked;
                                                    app.persist_prefs();
                                                    cx.notify();
                                                });
                                            }
                                        },
                                    ))
                                    .child(setting_toggle_row(
                                        "sw-stream",
                                        "Stream responses",
                                        current_prefs.streaming_enabled,
                                        palette,
                                        move |checked, _, cx| {
                                            if let Some(e) = this_stream.upgrade() {
                                                e.update(cx, |app, cx| {
                                                    app.prefs.streaming_enabled = *checked;
                                                    app.persist_prefs();
                                                    cx.notify();
                                                });
                                            }
                                        },
                                    ))
                                    .child(setting_toggle_row(
                                        "sw-overlay",
                                        "Floating HUD overlay after OCR",
                                        current_prefs.translation_overlay,
                                        palette,
                                        move |checked, _, cx| {
                                            if let Some(e) = this_overlay.upgrade() {
                                                e.update(cx, |app, cx| {
                                                    app.prefs.translation_overlay = *checked;
                                                    app.persist_prefs();
                                                    cx.notify();
                                                });
                                            }
                                        },
                                    ))
                                    .child(setting_toggle_row(
                                        "sw-clipboard",
                                        "Auto-translate copied clipboard text",
                                        current_prefs.clipboard_watch,
                                        palette,
                                        move |checked, _, cx| {
                                            if let Some(e) = this_clipboard.upgrade() {
                                                e.update(cx, |app, cx| {
                                                    app.prefs.clipboard_watch = *checked;
                                                    app.persist_prefs();
                                                    cx.notify();
                                                });
                                            }
                                        },
                                    ))
                                    .child(setting_toggle_row(
                                        "sw-keychain",
                                        "Store API keys in OS Keychain",
                                        current_prefs.use_keychain,
                                        palette,
                                        move |checked, _, cx| {
                                            if let Some(e) = this_keychain.upgrade() {
                                                e.update(cx, |app, cx| {
                                                    app.prefs.use_keychain = *checked;
                                                    app.persist_prefs();
                                                    cx.notify();
                                                });
                                            }
                                        },
                                    )),
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
                                .bg(rgb(palette.raised))
                                .border_1()
                                .border_color(rgba(palette.border_hairline))
                                .text_sm()
                                .text_color(rgb(palette.text_secondary))
                                .cursor_pointer()
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
                                .bg(rgb(palette.primary_btn_bg))
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(palette.primary_btn_fg))
                                .cursor_pointer()
                                .on_click(move |_, window, cx| {
                                    if let Some(selected) =
                                        select_for_save.read(cx).selected_value()
                                    {
                                        let chosen =
                                            TranslatorProvider::from_label(selected.as_ref());
                                        let model_val = model_for_save.read(cx).value().to_string();
                                        let key_val = key_for_save.read(cx).value().to_string();
                                        let url_val = url_for_save.read(cx).value().to_string();

                                        if let Some(app_entity) = this_for_save.upgrade() {
                                            app_entity.update(cx, |app, cx| {
                                                chosen.save_settings(
                                                    &key_val,
                                                    &model_val,
                                                    &url_val,
                                                    app.prefs.use_keychain,
                                                );
                                                app.translator_provider = chosen;
                                                app.prefs.translator = chosen.label().to_string();
                                                app.persist_prefs();
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

    fn open_history_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Filter history"));
        let this_entity = cx.entity().downgrade();
        let input_entity = self.input_state.clone();

        window.open_dialog(cx, move |dialog, window, cx| {
            let query = search_input.read(cx).value().to_string();
            let (items, palette) = this_entity
                .upgrade()
                .map(|e| {
                    let app = e.read(cx);
                    (app.history.search(&query), app.palette(window))
                })
                .unwrap_or((Vec::new(), Palette::DARK));

            let this_clear = this_entity.clone();
            let this_csv = this_entity.clone();
            let this_md = this_entity.clone();

            let mut list = div().v_flex().w_full().gap(px(8.)).max_h(px(300.));
            if items.is_empty() {
                list = list.child(
                    div()
                        .py(px(24.))
                        .text_sm()
                        .text_color(rgb(palette.text_muted))
                        .child("No translation history yet"),
                );
            } else {
                for entry in items.into_iter().take(12) {
                    let this_pick = this_entity.clone();
                    let input_pick = input_entity.clone();
                    let entry_clone = entry.clone();

                    list = list.child(
                        div()
                            .id(SharedString::from(format!("hist-{}", entry.id)))
                            .v_flex()
                            .w_full()
                            .p(px(10.))
                            .gap(px(4.))
                            .rounded(px(8.))
                            .bg(rgb(palette.raised))
                            .border_1()
                            .border_color(rgba(palette.border_hairline))
                            .cursor_pointer()
                            .on_click(move |_, window, cx| {
                                input_pick.update(cx, |s, cx| {
                                    s.set_value(entry_clone.source_text.clone(), window, cx);
                                });
                                if let Some(app_entity) = this_pick.upgrade() {
                                    app_entity.update(cx, |app, cx| {
                                        app.result_primary = entry_clone.primary_english.clone();
                                        app.result_secondary =
                                            entry_clone.secondary_localized.clone();
                                        app.show_results = true;
                                        cx.notify();
                                    });
                                }
                                window.close_dialog(cx);
                            })
                            .child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(palette.text_muted))
                                            .child(entry.provider.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(palette.text_muted))
                                            .child(entry.timestamp.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(palette.text_primary))
                                    .child(entry.primary_english.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(palette.text_secondary))
                                    .child(entry.secondary_localized.clone()),
                            ),
                    );
                }
            }

            dialog
                .title("History")
                .w(px(372.))
                .centered()
                .child(
                    div()
                        .v_flex()
                        .w_full()
                        .gap(px(12.))
                        .py(px(8.))
                        .child(Input::new(&search_input).w_full().cleanable(true))
                        .child(list),
                )
                .footer(
                    div()
                        .h_flex()
                        .items_center()
                        .justify_between()
                        .w_full()
                        .pt(px(8.))
                        .child(
                            div()
                                .h_flex()
                                .items_center()
                                .gap(px(6.))
                                .child(
                                    Button::new("hist-export-csv")
                                        .ghost()
                                        .icon(IconName::FileDown)
                                        .label("CSV")
                                        .tooltip("Export CSV to Downloads")
                                        .on_click(move |_, window, cx| {
                                            if let Some(e) = this_csv.upgrade() {
                                                let path = paths::downloads_dir()
                                                    .join("translation_history.csv");
                                                if e.read(cx).history.export_to(&path).is_ok() {
                                                    cx.reveal_path(&path);
                                                    window.push_notification(
                                                        Notification::success("Exported CSV"),
                                                        cx,
                                                    );
                                                }
                                            }
                                        }),
                                )
                                .child(
                                    Button::new("hist-export-md")
                                        .ghost()
                                        .icon(IconName::FileDown)
                                        .label("MD")
                                        .tooltip("Export Markdown to Downloads")
                                        .on_click(move |_, window, cx| {
                                            if let Some(e) = this_md.upgrade() {
                                                let path = paths::downloads_dir()
                                                    .join("translation_history.md");
                                                if e.read(cx).history.export_to(&path).is_ok() {
                                                    cx.reveal_path(&path);
                                                    window.push_notification(
                                                        Notification::success("Exported Markdown"),
                                                        cx,
                                                    );
                                                }
                                            }
                                        }),
                                )
                                .child(
                                    Button::new("history-clear-btn")
                                        .ghost()
                                        .icon(IconName::Trash)
                                        .tooltip("Clear history")
                                        .on_click(move |_, window, cx| {
                                            if let Some(e) = this_clear.upgrade() {
                                                e.update(cx, |app, cx| {
                                                    app.history.clear();
                                                    cx.notify();
                                                });
                                            }
                                            window.push_notification(
                                                Notification::info("History cleared"),
                                                cx,
                                            );
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .id("history-close-btn")
                                .h_flex()
                                .items_center()
                                .justify_center()
                                .h(px(32.))
                                .px(px(16.))
                                .rounded(px(8.))
                                .bg(rgb(palette.primary_btn_bg))
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(palette.primary_btn_fg))
                                .cursor_pointer()
                                .on_click(|_, window, cx| {
                                    window.close_dialog(cx);
                                })
                                .child("Done"),
                        ),
                )
        });
    }

    fn open_glossary_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let source_input = cx.new(|cx| InputState::new(window, cx).placeholder("Source term"));
        let target_input = cx.new(|cx| InputState::new(window, cx).placeholder("Translation"));
        let this_entity = cx.entity().downgrade();

        window.open_dialog(cx, move |dialog, window, cx| {
            let (entries, palette) = this_entity
                .upgrade()
                .map(|e| {
                    let app = e.read(cx);
                    (app.glossary.entries().to_vec(), app.palette(window))
                })
                .unwrap_or((Vec::new(), Palette::DARK));

            let src_for_add = source_input.clone();
            let tgt_for_add = target_input.clone();
            let this_for_add = this_entity.clone();
            let this_export = this_entity.clone();
            let this_import = this_entity.clone();

            let mut list = div().v_flex().w_full().gap(px(6.)).max_h(px(220.));
            if entries.is_empty() {
                list = list.child(
                    div()
                        .py(px(16.))
                        .text_xs()
                        .text_color(rgb(palette.text_muted))
                        .child("Terms added here are injected into AI translation prompts"),
                );
            } else {
                for (idx, item) in entries.into_iter().enumerate() {
                    let this_del = this_entity.clone();
                    list = list.child(
                        div()
                            .h_flex()
                            .items_center()
                            .justify_between()
                            .w_full()
                            .px(px(10.))
                            .py(px(8.))
                            .rounded(px(8.))
                            .bg(rgb(palette.raised))
                            .border_1()
                            .border_color(rgba(palette.border_hairline))
                            .child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(palette.text_primary))
                                            .child(item.source),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(palette.text_muted))
                                            .child("→"),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(palette.text_secondary))
                                            .child(item.target),
                                    ),
                            )
                            .child(
                                Button::new(SharedString::from(format!("gloss-del-{idx}")))
                                    .ghost()
                                    .icon(IconName::Trash)
                                    .tooltip("Remove")
                                    .on_click(move |_, _, cx| {
                                        if let Some(e) = this_del.upgrade() {
                                            e.update(cx, |app, cx| {
                                                app.glossary.remove_at(idx);
                                                cx.notify();
                                            });
                                        }
                                    }),
                            ),
                    );
                }
            }

            dialog
                .title("Glossary")
                .w(px(372.))
                .centered()
                .child(
                    div()
                        .v_flex()
                        .w_full()
                        .gap(px(12.))
                        .py(px(8.))
                        .child(
                            div()
                                .h_flex()
                                .items_center()
                                .gap(px(8.))
                                .w_full()
                                .child(Input::new(&source_input).flex_1())
                                .child(Input::new(&target_input).flex_1())
                                .child(
                                    Button::new("glossary-add-btn")
                                        .ghost()
                                        .icon(IconName::Plus)
                                        .tooltip("Add term")
                                        .on_click(move |_, window, cx| {
                                            let s = src_for_add.read(cx).value().to_string();
                                            let t = tgt_for_add.read(cx).value().to_string();
                                            if let Some(e) = this_for_add.upgrade() {
                                                let added = e.update(cx, |app, cx| {
                                                    let ok = app.glossary.add_or_update(&s, &t);
                                                    cx.notify();
                                                    ok
                                                });
                                                if added {
                                                    src_for_add.update(cx, |st, cx| {
                                                        st.set_value("", window, cx);
                                                    });
                                                    tgt_for_add.update(cx, |st, cx| {
                                                        st.set_value("", window, cx);
                                                    });
                                                }
                                            }
                                        }),
                                ),
                        )
                        .child(list),
                )
                .footer(
                    div()
                        .h_flex()
                        .items_center()
                        .justify_between()
                        .w_full()
                        .pt(px(8.))
                        .child(
                            div()
                                .h_flex()
                                .items_center()
                                .gap(px(6.))
                                .child(
                                    Button::new("glossary-import-btn")
                                        .ghost()
                                        .icon(IconName::Upload)
                                        .label("Import")
                                        .tooltip("Import glossary.csv from Downloads")
                                        .on_click(move |_, window, cx| {
                                            let path = paths::downloads_dir().join("glossary.csv");
                                            if let Some(e) = this_import.upgrade() {
                                                let count = e.update(cx, |app, cx| {
                                                    let n = app
                                                        .glossary
                                                        .import_from(&path)
                                                        .unwrap_or(0);
                                                    cx.notify();
                                                    n
                                                });
                                                window.push_notification(
                                                    Notification::info(format!(
                                                        "Imported {count} terms from {}",
                                                        path.display()
                                                    )),
                                                    cx,
                                                );
                                            }
                                        }),
                                )
                                .child(
                                    Button::new("glossary-export-btn")
                                        .ghost()
                                        .icon(IconName::Download)
                                        .label("Export")
                                        .tooltip("Export glossary.csv to Downloads")
                                        .on_click(move |_, window, cx| {
                                            if let Some(e) = this_export.upgrade() {
                                                let path =
                                                    paths::downloads_dir().join("glossary.csv");
                                                if e.read(cx).glossary.export_to(&path).is_ok() {
                                                    cx.reveal_path(&path);
                                                    window.push_notification(
                                                        Notification::success("Exported glossary"),
                                                        cx,
                                                    );
                                                }
                                            }
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .id("glossary-done-btn")
                                .h_flex()
                                .items_center()
                                .justify_center()
                                .h(px(32.))
                                .px(px(16.))
                                .rounded(px(8.))
                                .bg(rgb(palette.primary_btn_bg))
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(palette.primary_btn_fg))
                                .cursor_pointer()
                                .on_click(|_, window, cx| {
                                    window.close_dialog(cx);
                                })
                                .child("Done"),
                        ),
                )
        });
    }

    fn open_change_language_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let langs: Vec<SharedString> = language_names()
            .into_iter()
            .map(SharedString::from)
            .collect();
        let current_lang = self.prefs.target_language.clone();
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

        window.open_dialog(cx, move |dialog, window, cx| {
            let palette = this_entity
                .upgrade()
                .map(|e| e.read(cx).palette(window))
                .unwrap_or(Palette::DARK);
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
                                .text_color(rgb(palette.text_secondary))
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
                                .bg(rgb(palette.raised))
                                .border_1()
                                .border_color(rgba(palette.border_hairline))
                                .text_sm()
                                .text_color(rgb(palette.text_secondary))
                                .cursor_pointer()
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
                                .bg(rgb(palette.primary_btn_bg))
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(palette.primary_btn_fg))
                                .cursor_pointer()
                                .on_click(move |_, window, cx| {
                                    if let Some(selected) = select_for_ok.read(cx).selected_value()
                                    {
                                        let new_lang = selected.to_string();
                                        if let Some(app_entity) = this_for_ok.upgrade() {
                                            app_entity.update(cx, |app, cx| {
                                                app.prefs.target_language = new_lang.clone();
                                                app.persist_prefs();
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

fn setting_toggle_row(
    id: &'static str,
    label: &'static str,
    checked: bool,
    palette: Palette,
    on_click: impl Fn(&bool, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .h_flex()
        .items_center()
        .justify_between()
        .w_full()
        .child(
            div()
                .text_xs()
                .text_color(rgb(palette.text_secondary))
                .child(label),
        )
        .child(Switch::new(id).checked(checked).on_click(on_click))
}

fn iso_tag(code: impl Into<SharedString>, p: Palette) -> impl IntoElement {
    div()
        .h_flex()
        .items_center()
        .justify_center()
        .min_w(px(32.))
        .h(px(20.))
        .px(px(6.))
        .rounded(px(4.))
        .bg(rgb(p.raised))
        .border_1()
        .border_color(rgba(p.border_hairline))
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(p.text_secondary))
        .child(code.into())
}

fn keycap(label: &'static str, p: Palette) -> impl IntoElement {
    div()
        .h_flex()
        .items_center()
        .justify_center()
        .h(px(20.))
        .px(px(6.))
        .rounded(px(4.))
        .bg(rgb(p.raised))
        .border_1()
        .border_color(rgba(p.border_subtle))
        .text_xs()
        .text_color(rgb(p.text_muted))
        .child(label)
}

impl Render for TranslateApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.palette(window);
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
        let pinned = self.prefs.always_on_top;
        let update_info = self.update_info.clone();

        div()
            .id("translate-app-root")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &TriggerOcr, window, cx| {
                this.start_ocr(window, cx);
            }))
            .v_flex()
            .justify_between()
            .size_full()
            .bg(rgb(p.page))
            .text_color(rgb(p.text_primary))
            .p(px(20.))
            .gap(px(16.))
            // 1. Top Header Bar
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
                            .text_color(rgb(p.text_primary))
                            .child("Translate App with AI"),
                    )
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap(px(4.))
                            .child(
                                Button::new("pin-btn")
                                    .ghost()
                                    .icon(if pinned {
                                        IconName::Pin
                                    } else {
                                        IconName::PinOff
                                    })
                                    .tooltip(if pinned {
                                        "Pinned on top"
                                    } else {
                                        "Pin on top"
                                    })
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.prefs.always_on_top = !this.prefs.always_on_top;
                                        this.persist_prefs();
                                        let _ =
                                            set_always_on_top(window, this.prefs.always_on_top);
                                        let msg = if this.prefs.always_on_top {
                                            "Always on top enabled"
                                        } else {
                                            "Always on top disabled"
                                        };
                                        window.push_notification(Notification::info(msg), cx);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("history-btn")
                                    .ghost()
                                    .icon(IconName::Clock)
                                    .tooltip("History")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_history_dialog(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("glossary-btn")
                                    .ghost()
                                    .icon(IconName::BookOpen)
                                    .tooltip("Glossary")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_glossary_dialog(window, cx);
                                    })),
                            ),
                    ),
            )
            // 2. Composer Card
            .child(
                div()
                    .v_flex()
                    .w_full()
                    .p(px(12.))
                    .gap(px(12.))
                    .rounded(px(12.))
                    .bg(rgb(p.card))
                    .border_1()
                    .border_color(rgba(p.border_hairline))
                    .child(Input::new(&self.input_state).w_full().cleanable(true))
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .justify_between()
                            .w_full()
                            .gap(px(8.))
                            .child(
                                div()
                                    .id("ocr-capture-action")
                                    .h_flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .h(px(36.))
                                    .px(px(12.))
                                    .rounded(px(8.))
                                    .bg(rgb(p.raised))
                                    .border_1()
                                    .border_color(rgba(p.border_hairline))
                                    .cursor_pointer()
                                    .hover(|s| s.border_color(rgba(p.border_subtle)))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.start_ocr(window, cx);
                                    }))
                                    .child(
                                        Icon::new(IconName::Crop)
                                            .size(px(16.))
                                            .text_color(rgb(p.text_secondary)),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(rgb(p.text_primary))
                                            .child("Capture"),
                                    )
                                    .child(keycap(ocr_shortcut, p)),
                            )
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
                                    .bg(rgb(p.primary_btn_bg))
                                    .text_color(rgb(p.primary_btn_fg))
                                    .cursor_pointer()
                                    .hover(|s| s.opacity(0.92))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.start_translate(window, cx);
                                    }))
                                    .child(
                                        Icon::new(IconName::Languages)
                                            .size(px(16.))
                                            .text_color(rgb(p.primary_btn_fg)),
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
                    .when(self.is_loading, |card| {
                        card.child(
                            Progress::new("translate-progress")
                                .w_full()
                                .loading(true)
                                .color(rgb(p.accent)),
                        )
                    }),
            )
            // 3. Moment 1: The Bilingual Translation Sheet
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .w_full()
                    .rounded(px(12.))
                    .bg(rgb(p.card))
                    .border_1()
                    .border_color(rgba(p.border_hairline))
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
                                        .child(iso_tag("EN", p))
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(p.text_muted))
                                                .child("→"),
                                        )
                                        .child(iso_tag(target_iso.clone(), p)),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(p.text_secondary))
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
                                        .child(keycap(ocr_shortcut, p))
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(p.text_muted))
                                                .child("Global region OCR capture"),
                                        ),
                                ),
                        )
                    })
                    .when(self.show_results, |sheet| {
                        sheet
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
                                                    .child(iso_tag("EN", p))
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(rgb(p.text_muted))
                                                            .child("English"),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .h_flex()
                                                    .items_center()
                                                    .gap(px(2.))
                                                    .child(
                                                        Button::new("tts-primary-btn")
                                                            .ghost()
                                                            .icon(IconName::Volume2)
                                                            .tooltip("Speak")
                                                            .on_click({
                                                                let text = primary_text.clone();
                                                                move |_, _, _| {
                                                                    speak_text(&text, "en");
                                                                }
                                                            }),
                                                    )
                                                    .child(
                                                        Button::new("copy-primary-btn")
                                                            .ghost()
                                                            .icon(IconName::Copy)
                                                            .tooltip("Copy")
                                                            .on_click(cx.listener({
                                                                let text = primary_text.clone();
                                                                move |this, _, window, cx| {
                                                                    this.clipboard_watcher
                                                                        .mark_self_copy(&text);
                                                                    copy_to_clipboard(&text);
                                                                    window.push_notification(
                                                                        Notification::success(
                                                                            "Copied",
                                                                        ),
                                                                        cx,
                                                                    );
                                                                }
                                                            })),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(p.text_primary))
                                            .child(primary_text.clone()),
                                    ),
                            )
                            .child(div().w_full().h(px(1.)).bg(rgba(p.border_hairline)))
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
                                                    .child(iso_tag(target_iso.clone(), p))
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(rgb(p.text_secondary))
                                                            .child(target_name.clone()),
                                                    )
                                                    .child(
                                                        Icon::new(IconName::Pencil)
                                                            .size(px(14.))
                                                            .text_color(rgb(p.text_muted)),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .h_flex()
                                                    .items_center()
                                                    .gap(px(2.))
                                                    .child(
                                                        Button::new("tts-secondary-btn")
                                                            .ghost()
                                                            .icon(IconName::Volume2)
                                                            .tooltip("Speak")
                                                            .on_click({
                                                                let text = secondary_text.clone();
                                                                let code = target_iso.clone();
                                                                move |_, _, _| {
                                                                    speak_text(&text, &code);
                                                                }
                                                            }),
                                                    )
                                                    .child(
                                                        Button::new("copy-secondary-btn")
                                                            .ghost()
                                                            .icon(IconName::Copy)
                                                            .tooltip("Copy")
                                                            .on_click(cx.listener({
                                                                let text = secondary_text.clone();
                                                                move |this, _, window, cx| {
                                                                    this.clipboard_watcher
                                                                        .mark_self_copy(&text);
                                                                    copy_to_clipboard(&text);
                                                                    window.push_notification(
                                                                        Notification::success(
                                                                            "Copied",
                                                                        ),
                                                                        cx,
                                                                    );
                                                                }
                                                            })),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(p.text_primary))
                                            .child(secondary_text.clone()),
                                    ),
                            )
                    }),
            )
            // 4. Moment 2: Quiet Bottom Utility Dock
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
                                            .text_color(rgb(p.text_muted))
                                            .child("OCR"),
                                    )
                                    .child(Select::new(&self.ocr_lang_select).flex_1()),
                            )
                            .child(
                                div()
                                    .id("engine-chip")
                                    .h_flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .h(px(32.))
                                    .px(px(10.))
                                    .rounded(px(8.))
                                    .bg(rgb(p.card))
                                    .border_1()
                                    .border_color(rgba(p.border_hairline))
                                    .cursor_pointer()
                                    .hover(|s| s.bg(rgb(p.raised)))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.open_settings_dialog(window, cx);
                                    }))
                                    .child(
                                        div()
                                            .size(px(6.))
                                            .rounded_full()
                                            .bg(rgb(p.accent)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(p.text_secondary))
                                            .child(provider_label),
                                    ),
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
                                    .text_color(rgb(p.text_muted))
                                    .child(
                                        self.status_hint
                                            .clone()
                                            .unwrap_or_else(|| format!("Target: {target_name}")),
                                    ),
                            )
                            .child(
                                div()
                                    .h_flex()
                                    .items_center()
                                    .gap(px(10.))
                                    .when_some(update_info, |row, info| {
                                        let info_clone = info.clone();
                                        row.child(
                                            div()
                                                .id("update-link")
                                                .h_flex()
                                                .items_center()
                                                .gap(px(4.))
                                                .cursor_pointer()
                                                .text_xs()
                                                .text_color(rgb(p.accent))
                                                .on_click(cx.listener(move |this, _, window, cx| {
                                                    this.download_update(
                                                        info_clone.clone(),
                                                        window,
                                                        cx,
                                                    );
                                                }))
                                                .child(format!("Update {}", info.latest_version))
                                                .child(
                                                    Icon::new(IconName::Download)
                                                        .size(px(12.))
                                                        .text_color(rgb(p.accent)),
                                                ),
                                        )
                                    })
                                    .child(
                                        div()
                                            .id("footer-link")
                                            .cursor_pointer()
                                            .text_xs()
                                            .text_color(rgb(p.text_muted))
                                            .hover(|s| s.text_color(rgb(p.text_secondary)))
                                            .on_click(|_, _, cx| {
                                                cx.open_url(
                                                    "https://github.com/watchakorn-18k/Language-Translation-App-using-CHATGPT",
                                                );
                                            })
                                            .child(format!("wk18k © {current_year}")),
                                    ),
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
            IconName::Clock,
            IconName::BookOpen,
            IconName::Pin,
            IconName::PinOff,
            IconName::Trash,
            IconName::Plus,
            IconName::ArrowUpRight,
            IconName::Volume2,
            IconName::FileDown,
            IconName::Upload,
            IconName::Download,
            IconName::X,
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
