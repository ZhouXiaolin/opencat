use crate::{scene::script::ScriptColor, style::ColorToken};

pub(crate) fn skia_color(token: ColorToken) -> skia_safe::Color {
    let (r, g, b, a) = token.rgba();
    skia_safe::Color::from_argb(a, r, g, b)
}

pub(crate) fn script_color(color: ScriptColor) -> skia_safe::Color {
    skia_safe::Color::from_argb(color.a, color.r, color.g, color.b)
}
