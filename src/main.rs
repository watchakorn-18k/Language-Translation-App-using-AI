use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;
use language_translation_app::hotkey::GlobalOcrHotkey;
use language_translation_app::providers::load_app_preferences;
use language_translation_app::snipper_overlay::CancelSnip;
use language_translation_app::translation_overlay::CloseTranslationOverlay;
use language_translation_app::ui::{TranslateApp, TriggerOcr};
use language_translation_app::window_level::set_always_on_top;

fn main() {
    // Register global hotkey on the main thread (required by macOS Carbon/AppKit event loop).
    let global_hotkey = GlobalOcrHotkey::register();

    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);

            let prefs = load_app_preferences();
            let system_dark = matches!(
                cx.window_appearance(),
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            );
            let mode = if prefs.theme.is_dark(system_dark) {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            };
            Theme::change(mode, None, cx);

            cx.bind_keys([
                KeyBinding::new("ctrl-shift-z", TriggerOcr, None),
                KeyBinding::new("cmd-shift-z", TriggerOcr, None),
                KeyBinding::new("escape", CancelSnip, None),
                KeyBinding::new("escape", CloseTranslationOverlay, None),
            ]);

            let bounds = Bounds::centered(None, size(px(420.), px(680.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some(SharedString::from("Translate App with AI | wk-18k")),
                    ..Default::default()
                }),
                ..Default::default()
            };

            let always_on_top = prefs.always_on_top;
            gpui_kit::open_window(options, cx, move |window, cx| {
                let _ = set_always_on_top(window, always_on_top);
                cx.new(|cx| TranslateApp::new(window, global_hotkey, cx))
            })
            .expect("failed to open Translate App window");
        });
}
