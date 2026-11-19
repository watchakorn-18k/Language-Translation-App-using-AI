use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;
use language_translation_app::snipper_overlay::CancelSnip;
use language_translation_app::ui::{TranslateApp, TriggerOcr};

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(|cx| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);

            cx.bind_keys([
                KeyBinding::new("ctrl-shift-z", TriggerOcr, None),
                KeyBinding::new("cmd-shift-z", TriggerOcr, None),
                KeyBinding::new("escape", CancelSnip, None),
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

            gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| TranslateApp::new(window, cx))
            })
            .expect("failed to open Translate App window");
        });
}
