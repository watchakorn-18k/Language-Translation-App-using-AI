use crate::snipper::NormalizedRect;
use gpui_kit::*;
use std::path::PathBuf;

actions!(snipper_overlay, [CancelSnip]);

pub type SnipCallback = Box<dyn FnOnce(Option<(PathBuf, NormalizedRect)>, &mut App) + 'static>;

/// Native GPUI fullscreen drag-capture overlay.
///
/// Direct Rust/gpui-kit port of `Snipper` and `OneTimeSnipper` from `src/Snippers.py`:
/// - Displays the captured screen frozen edge-to-edge under a translucent dim veil
/// - Uses a crosshair cursor (`CursorStyle::Crosshair`)
/// - Click-and-drag draws a crisp un-dimmed selection rectangle (`#0074ff`, 3px border)
/// - Releasing the mouse crops that rectangle into `orc.png` and triggers OCR
/// - Pressing `Escape` cancels and closes the overlay
pub struct SnipperOverlay {
    screenshot_path: PathBuf,
    focus_handle: FocusHandle,
    start: Option<Point<Pixels>>,
    end: Option<Point<Pixels>>,
    dragging: bool,
    on_finish: Option<SnipCallback>,
}

impl SnipperOverlay {
    pub fn new(
        screenshot_path: PathBuf,
        on_finish: SnipCallback,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);

        Self {
            screenshot_path,
            focus_handle,
            start: None,
            end: None,
            dragging: false,
            on_finish: Some(on_finish),
        }
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let _ = std::fs::remove_file(&self.screenshot_path);
        if let Some(cb) = self.on_finish.take() {
            cb(None, cx);
        }
        window.remove_window();
    }

    fn finish_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let viewport = window.viewport_size();
        let vw: f32 = viewport.width.into();
        let vh: f32 = viewport.height.into();

        let selection = match (self.start, self.end) {
            (Some(s), Some(e)) => {
                let sx: f32 = s.x.into();
                let sy: f32 = s.y.into();
                let ex: f32 = e.x.into();
                let ey: f32 = e.y.into();
                NormalizedRect::from_points(sx, sy, ex, ey, vw, vh)
            }
            _ => None,
        };

        if let Some(rect) = selection {
            let path = self.screenshot_path.clone();
            if let Some(cb) = self.on_finish.take() {
                cb(Some((path, rect)), cx);
            }
            window.remove_window();
        } else {
            self.start = None;
            self.end = None;
            self.dragging = false;
            cx.notify();
        }
    }
}

impl Render for SnipperOverlay {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let vw: f32 = viewport.width.into();
        let vh: f32 = viewport.height.into();

        // Dark translucent veil matching QColor(0, 0, 0, 100) from src/Snippers.py
        let dim = rgba(0x00000064);

        let selection_box = match (self.start, self.end) {
            (Some(s), Some(e)) => {
                let sx: f32 = s.x.into();
                let sy: f32 = s.y.into();
                let ex: f32 = e.x.into();
                let ey: f32 = e.y.into();
                let x1 = sx.min(ex).clamp(0.0, vw);
                let y1 = sy.min(ey).clamp(0.0, vh);
                let x2 = sx.max(ex).clamp(0.0, vw);
                let y2 = sy.max(ey).clamp(0.0, vh);
                if (x2 - x1) >= 2.0 && (y2 - y1) >= 2.0 {
                    Some((x1, y1, x2 - x1, y2 - y1))
                } else {
                    None
                }
            }
            _ => None,
        };

        let mut root = div()
            .id("snipper-overlay-root")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &CancelSnip, window, cx| {
                this.cancel(window, cx);
            }))
            .relative()
            .size_full()
            .overflow_hidden()
            .cursor(CursorStyle::Crosshair)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                    this.start = Some(event.position);
                    this.end = Some(event.position);
                    this.dragging = true;
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
                if this.dragging {
                    this.end = Some(event.position);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &MouseUpEvent, window, cx| {
                    if this.dragging {
                        this.end = Some(event.position);
                        this.dragging = false;
                        this.finish_selection(window, cx);
                    }
                }),
            )
            // Frozen fullscreen background screenshot
            .child(
                img(self.screenshot_path.clone())
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .object_fit(ObjectFit::Fill),
            );

        if let Some((x, y, w, h)) = selection_box {
            let right_x = x + w;
            let bottom_y = y + h;
            let right_w = (vw - right_x).max(0.0);
            let bottom_h = (vh - bottom_y).max(0.0);

            // 4 dim strips around the selection box so the selected region itself stays crystal clear
            root = root
                // Top dim strip
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .w(px(vw))
                        .h(px(y))
                        .bg(dim),
                )
                // Bottom dim strip
                .child(
                    div()
                        .absolute()
                        .top(px(bottom_y))
                        .left_0()
                        .w(px(vw))
                        .h(px(bottom_h))
                        .bg(dim),
                )
                // Left dim strip
                .child(
                    div()
                        .absolute()
                        .top(px(y))
                        .left_0()
                        .w(px(x))
                        .h(px(h))
                        .bg(dim),
                )
                // Right dim strip
                .child(
                    div()
                        .absolute()
                        .top(px(y))
                        .left(px(right_x))
                        .w(px(right_w))
                        .h(px(h))
                        .bg(dim),
                )
                // Selection border matching QColor(0, 116, 255), 3px from src/Snippers.py
                .child(
                    div()
                        .absolute()
                        .top(px(y))
                        .left(px(x))
                        .w(px(w))
                        .h(px(h))
                        .border(px(3.))
                        .border_color(rgb(0x0074ff)),
                )
                // Compact live dimension badge
                .child(
                    div()
                        .absolute()
                        .top(px((y - 28.0).max(8.0)))
                        .left(px(x))
                        .px(px(8.))
                        .py(px(3.))
                        .rounded(px(4.))
                        .bg(rgba(0x141415e6))
                        .border_1()
                        .border_color(rgba(0xededed22))
                        .text_xs()
                        .text_color(rgb(0xededed))
                        .child(format!("{} × {}", w.round() as u32, h.round() as u32)),
                );
        } else {
            // Fullscreen dim veil + quiet instruction pill before drag starts
            root = root.child(div().absolute().top_0().left_0().size_full().bg(dim)).child(
                div()
                    .absolute()
                    .top(px(28.))
                    .left(px((vw / 2.0 - 165.0).max(16.0)))
                    .px(px(14.))
                    .py(px(8.))
                    .rounded(px(8.))
                    .bg(rgba(0x141415eb))
                    .border_1()
                    .border_color(rgba(0xededed1f))
                    .text_sm()
                    .text_color(rgb(0xededed))
                    .child("Drag to select region for OCR  ·  Esc to cancel"),
            );
        }

        root
    }
}

/// Opens the frameless fullscreen drag-capture overlay window on the primary display.
pub fn open_snipper_overlay(
    screenshot_path: PathBuf,
    on_finish: SnipCallback,
    cx: &mut App,
) -> std::result::Result<(), String> {
    let display = cx
        .primary_display()
        .ok_or_else(|| "No primary display found".to_string())?;
    let bounds = display.bounds();

    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        focus: true,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: false,
        ..Default::default()
    };

    cx.open_window(options, |window, cx| {
        cx.new(|cx| SnipperOverlay::new(screenshot_path, on_finish, window, cx))
    })
    .map_err(|e| e.to_string())?;

    Ok(())
}
