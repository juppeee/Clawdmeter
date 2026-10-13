//! The handful of controls the windows need, drawn with the design tokens:
//! buttons, pills, toggles, segmented choices, setting rows, and Clawd.

use gpui::{
    canvas, div, fill, point, prelude::*, px, rgb, size, App, Bounds, ClickEvent, ElementId, Hsla, Rgba, SharedString,
    Window,
};

use super::theme::{Theme, FONT_DISPLAY, FONT_MONO, R_M, R_PILL, R_S};
use crate::art::{cell_rgb, idle_cell, IDLE_H, IDLE_W};

type Handler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// Button look.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    /// Accent fill: the one action that moves things forward.
    Primary,
    /// Hairline outline.
    Normal,
    /// Hairline outline, compact.
    Small,
}

/// A pill-shaped button.
pub fn button(
    t: &Theme,
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    kind: ButtonKind,
    enabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let (bg, fg, border) = match kind {
        ButtonKind::Primary => (t.accent, t.accent_ink, t.accent),
        _ => (t.surface, t.text, t.line),
    };
    let hover_bg = match kind {
        ButtonKind::Primary => t.focus,
        _ => t.panel,
    };
    let (pad_x, pad_y, size) = if kind == ButtonKind::Small { (12., 4., 13.) } else { (16., 7., 14.) };
    let el = div()
        .id(id.into())
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .px(px(pad_x))
        .py(px(pad_y))
        .rounded(px(R_PILL))
        .border_1()
        .border_color(border)
        .bg(bg)
        .text_color(fg)
        .text_size(px(size))
        .whitespace_nowrap()
        .child(label.into());
    if enabled {
        let handler: Handler = Box::new(on_click);
        el.cursor_pointer().hover(move |s| s.bg(hover_bg)).on_click(move |e, w, cx| handler(e, w, cx))
    } else {
        el.opacity(0.45)
    }
}

/// Semantic tone for pills and dots.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// Neutral.
    Plain,
    /// Clawd / in progress.
    Accent,
    /// Good.
    Ok,
    /// Needs a look.
    Warn,
    /// Broken.
    Bad,
}

/// Colour for a tone.
pub fn tone_color(t: &Theme, tone: Tone) -> Rgba {
    match tone {
        Tone::Plain => t.dim,
        Tone::Accent => t.accent,
        Tone::Ok => t.ok,
        Tone::Warn => t.warn,
        Tone::Bad => t.bad,
    }
}

fn tint(c: Rgba, alpha: f32) -> Hsla {
    let mut h: Hsla = c.into();
    h.a = alpha;
    h
}

/// A small status chip.
pub fn pill(t: &Theme, text: impl Into<SharedString>, tone: Tone) -> impl IntoElement {
    let c = tone_color(t, tone);
    div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap(px(6.))
        .px(px(10.))
        .py(px(3.))
        .rounded(px(R_PILL))
        .bg(tint(c, if tone == Tone::Plain { 0.12 } else { 0.16 }))
        .text_color(if tone == Tone::Plain { t.dim } else { c })
        .text_size(px(12.))
        .whitespace_nowrap()
        .child(div().size(px(6.)).rounded(px(R_PILL)).bg(c))
        .child(text.into())
}

/// A round status dot.
pub fn dot(t: &Theme, tone: Tone) -> impl IntoElement {
    div().flex_shrink_0().size(px(8.)).rounded(px(R_PILL)).bg(tone_color(t, tone))
}

/// An on/off switch.
pub fn toggle(
    t: &Theme,
    id: impl Into<ElementId>,
    on: bool,
    enabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let track = if on { t.accent } else { t.track };
    let knob = if on { t.accent_ink } else { t.dim };
    let el = div()
        .id(id.into())
        .flex()
        .flex_shrink_0()
        .items_center()
        .w(px(38.))
        .h(px(22.))
        .p(px(3.))
        .rounded(px(R_PILL))
        .bg(track)
        .when(on, |d| d.justify_end())
        .child(div().size(px(16.)).rounded(px(R_PILL)).bg(knob));
    if enabled {
        let handler: Handler = Box::new(on_click);
        el.cursor_pointer().on_click(move |e, w, cx| handler(e, w, cx))
    } else {
        el.opacity(0.45)
    }
}

/// A row of mutually exclusive choices (stands in for a select).
pub fn segmented<V: Clone + PartialEq + 'static>(
    t: &Theme,
    id: &str,
    options: &[(V, &str)],
    selected: &V,
    enabled: bool,
    on_select: impl Fn(V, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let on_select = std::rc::Rc::new(on_select);
    let mut row = div().flex().flex_shrink_0().p(px(2.)).gap(px(2.)).rounded(px(R_S + 2.)).bg(t.track);
    for (i, (value, label)) in options.iter().enumerate() {
        let on = value == selected;
        let v = value.clone();
        let cb = on_select.clone();
        let mut seg = div()
            .id(SharedString::from(format!("{id}-{i}")))
            .px(px(10.))
            .py(px(4.))
            .rounded(px(R_S))
            .text_size(px(13.))
            .whitespace_nowrap()
            .text_color(if on { t.text } else { t.dim })
            .when(on, |d| d.bg(t.surface))
            .child(SharedString::from(label.to_string()));
        if enabled && !on {
            seg = seg.cursor_pointer().on_click(move |_, w, cx| cb(v.clone(), w, cx));
        }
        row = row.child(seg);
    }
    if !enabled {
        row = row.opacity(0.45);
    }
    row
}

/// A settings row: label + help on the left, the control on the right.
pub fn field(
    t: &Theme,
    label: impl Into<SharedString>,
    help: impl Into<SharedString>,
    control: impl IntoElement,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(16.))
        .py(px(12.))
        .border_b_1()
        .border_color(t.line)
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(3.))
                .child(div().text_size(px(14.)).text_color(t.text).child(label.into()))
                .child(div().text_size(px(12.5)).text_color(t.mute).child(help.into())),
        )
        .child(div().flex_shrink_0().child(control))
}

/// Section heading in the display face.
pub fn heading(t: &Theme, text: impl Into<SharedString>) -> impl IntoElement {
    div().font_family(FONT_DISPLAY).text_size(px(20.)).text_color(t.text).child(text.into())
}

/// Secondary paragraph text.
pub fn para(t: &Theme, text: impl Into<SharedString>) -> impl IntoElement {
    div().text_size(px(13.5)).line_height(px(20.)).text_color(t.dim).child(text.into())
}

/// Inline data in the mono face.
pub fn mono(t: &Theme, text: impl Into<SharedString>) -> impl IntoElement {
    div().font_family(FONT_MONO).text_size(px(12.5)).text_color(t.dim).child(text.into())
}

/// A panel card.
pub fn card(t: &Theme) -> gpui::Div {
    div().flex().flex_col().p(px(16.)).gap(px(10.)).rounded(px(R_M)).bg(t.panel)
}

/// Key/value rows (`dl`).
pub fn kv(t: &Theme, rows: Vec<(&'static str, SharedString)>) -> impl IntoElement {
    let mut d = div().flex().flex_col().gap(px(6.)).flex_1();
    for (k, v) in rows {
        d = d.child(
            div()
                .flex()
                .gap(px(12.))
                .child(div().w(px(76.)).flex_shrink_0().text_size(px(13.)).text_color(t.mute).child(k))
                .child(div().flex_1().text_size(px(13.)).text_color(t.text).child(v)),
        );
    }
    d
}

/// The official idle Clawd at `cell` px per cell (whole cells, no smoothing).
pub fn clawd(cell: f32) -> impl IntoElement {
    div().flex_shrink_0().w(px(cell * IDLE_W as f32)).h(px(cell * IDLE_H as f32)).child(
        canvas(
            |_, _, _| (),
            move |bounds: Bounds<gpui::Pixels>, _, window, _| {
                for r in 0..IDLE_H {
                    for c in 0..IDLE_W {
                        let v = idle_cell(c, r);
                        if v == 0 {
                            continue;
                        }
                        let origin = bounds.origin + point(px(c as f32 * cell), px(r as f32 * cell));
                        window.paint_quad(fill(Bounds { origin, size: size(px(cell), px(cell)) }, rgb(cell_rgb(v))));
                    }
                }
            },
        )
        .size_full(),
    )
}

/// A board thumbnail: the panel at true aspect ratio inside a bezel, with a
/// 1 px-cell Clawd on it, in a `box_px` square.
pub fn board_thumb(t: &Theme, panel: (f32, f32), box_px: f32) -> impl IntoElement {
    let (w, h) = panel;
    div().flex().flex_shrink_0().items_center().justify_center().size(px(box_px)).child(
        div().p(px(2.)).rounded(px(4.)).bg(t.track).child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w(px(w))
                .h(px(h))
                .rounded(px(2.))
                .bg(rgb(0x000000))
                .overflow_hidden()
                .child(clawd(if w.min(h) >= 60. { 2. } else { 1. })),
        ),
    )
}
