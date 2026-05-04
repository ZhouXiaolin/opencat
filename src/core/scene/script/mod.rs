pub mod host;
pub mod mutations;

pub use host::{ScriptDriverId, ScriptHost};
pub use mutations::*;

use crate::core::style::{
    AlignItems, BoxShadow, BoxShadowStyle, DropShadow, DropShadowStyle, FlexDirection,
    InsetShadow, InsetShadowStyle, JustifyContent, ObjectFit, Position, TextAlign,
};

#[derive(Clone, Debug, Default)]
pub struct ScriptDriver {
    pub(crate) source: String,
}

impl ScriptDriver {
    pub fn from_source(source: &str) -> anyhow::Result<Self> {
        Ok(Self {
            source: source.to_string(),
        })
    }

    pub(crate) fn cache_key(&self) -> u64 {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut h = DefaultHasher::new();
        self.source.hash(&mut h);
        h.finish()
    }
}

pub fn driver_from_source(source: &str) -> anyhow::Result<ScriptDriver> {
    ScriptDriver::from_source(source)
}

#[derive(Debug, Clone)]
pub struct ScriptTextSource {
    pub text: String,
    pub kind: ScriptTextSourceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptTextSourceKind {
    TextNode,
    Caption,
}

pub(crate) fn position_from_name(name: &str) -> Option<Position> {
    match name {
        "relative" => Some(Position::Relative),
        "absolute" => Some(Position::Absolute),
        _ => None,
    }
}

pub(crate) fn flex_direction_from_name(name: &str) -> Option<FlexDirection> {
    match name {
        "row" => Some(FlexDirection::Row),
        "col" | "column" => Some(FlexDirection::Col),
        _ => None,
    }
}

pub(crate) fn justify_content_from_name(name: &str) -> Option<JustifyContent> {
    match name {
        "start" => Some(JustifyContent::Start),
        "center" => Some(JustifyContent::Center),
        "end" => Some(JustifyContent::End),
        "between" => Some(JustifyContent::Between),
        "around" => Some(JustifyContent::Around),
        "evenly" => Some(JustifyContent::Evenly),
        _ => None,
    }
}

pub(crate) fn align_items_from_name(name: &str) -> Option<AlignItems> {
    match name {
        "start" => Some(AlignItems::Start),
        "center" => Some(AlignItems::Center),
        "end" => Some(AlignItems::End),
        "stretch" => Some(AlignItems::Stretch),
        _ => None,
    }
}

pub(crate) fn object_fit_from_name(name: &str) -> Option<ObjectFit> {
    match name {
        "contain" => Some(ObjectFit::Contain),
        "cover" => Some(ObjectFit::Cover),
        "fill" => Some(ObjectFit::Fill),
        _ => None,
    }
}

pub(crate) fn box_shadow_from_name(name: &str) -> Option<BoxShadow> {
    match name {
        "2xs" => Some(BoxShadow::from_style(BoxShadowStyle::TwoXs)),
        "xs" => Some(BoxShadow::from_style(BoxShadowStyle::Xs)),
        "sm" => Some(BoxShadow::from_style(BoxShadowStyle::Sm)),
        "base" | "default" => Some(BoxShadow::from_style(BoxShadowStyle::Base)),
        "md" => Some(BoxShadow::from_style(BoxShadowStyle::Md)),
        "lg" => Some(BoxShadow::from_style(BoxShadowStyle::Lg)),
        "xl" => Some(BoxShadow::from_style(BoxShadowStyle::Xl)),
        "2xl" => Some(BoxShadow::from_style(BoxShadowStyle::TwoXl)),
        "3xl" => Some(BoxShadow::from_style(BoxShadowStyle::ThreeXl)),
        _ => None,
    }
}

pub(crate) fn inset_shadow_from_name(name: &str) -> Option<InsetShadow> {
    match name {
        "2xs" => Some(InsetShadow::from_style(InsetShadowStyle::TwoXs)),
        "xs" => Some(InsetShadow::from_style(InsetShadowStyle::Xs)),
        "base" | "default" => Some(InsetShadow::from_style(InsetShadowStyle::Base)),
        "sm" => Some(InsetShadow::from_style(InsetShadowStyle::Sm)),
        "md" => Some(InsetShadow::from_style(InsetShadowStyle::Md)),
        _ => None,
    }
}

pub(crate) fn drop_shadow_from_name(name: &str) -> Option<DropShadow> {
    match name {
        "xs" => Some(DropShadow::from_style(DropShadowStyle::Xs)),
        "sm" => Some(DropShadow::from_style(DropShadowStyle::Sm)),
        "base" | "default" => Some(DropShadow::from_style(DropShadowStyle::Base)),
        "md" => Some(DropShadow::from_style(DropShadowStyle::Md)),
        "lg" => Some(DropShadow::from_style(DropShadowStyle::Lg)),
        "xl" => Some(DropShadow::from_style(DropShadowStyle::Xl)),
        "2xl" => Some(DropShadow::from_style(DropShadowStyle::TwoXl)),
        "3xl" => Some(DropShadow::from_style(DropShadowStyle::ThreeXl)),
        _ => None,
    }
}

pub(crate) fn text_align_from_name(name: &str) -> Option<TextAlign> {
    match name {
        "left" => Some(TextAlign::Left),
        "center" => Some(TextAlign::Center),
        "right" => Some(TextAlign::Right),
        _ => None,
    }
}

#[cfg(feature = "host-default")]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::style::{ColorToken, Transform};

    #[test]
    fn script_driver_records_text_alignment_and_line_height() {
        let driver = ScriptDriver::from_source(
            r#"
            const title = ctx.getNode("title");
            title.textAlign("center").lineHeight(1.8);
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(0, 1, 0, 1, None).expect("script should run");
        let title = mutations.get("title").expect("title mutation should exist");

        assert_eq!(title.text_align, Some(TextAlign::Center));
        assert_eq!(title.line_height, Some(1.8));
    }

    #[test]
    fn script_driver_exposes_global_and_scene_frame_fields() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.getNode("box")
                .translateX(ctx.frame + ctx.totalFrames)
                .translateY(ctx.currentFrame + ctx.sceneFrames);
        "#,
        )
        .expect("script should compile");

        let mutations = driver
            .run(12, 240, 3, 30, Some("box"))
            .expect("script should run");
        let node = mutations.get("box").expect("box mutation should exist");

        assert_eq!(
            node.transforms,
            vec![Transform::TranslateX(252.0), Transform::TranslateY(33.0)]
        );
    }

    #[test]
    fn script_driver_preserves_transform_call_order() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.getNode("box")
                .translateX(40)
                .rotate(15)
                .scale(1.2);
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(0, 1, 0, 1, None).expect("script should run");
        let node = mutations.get("box").expect("box mutation should exist");

        assert_eq!(
            node.transforms,
            vec![
                Transform::TranslateX(40.0),
                Transform::RotateDeg(15.0),
                Transform::Scale(1.2),
            ]
        );
    }

    #[test]
    fn script_driver_records_lucide_fill_and_stroke() {
        let driver = ScriptDriver::from_source(
            r#"
            ctx.getNode("icon")
                .strokeColor("blue")
                .strokeWidth(3)
                .fillColor("sky200");
        "#,
        )
        .expect("script should compile");

        let mutations = driver.run(0, 1, 0, 1, None).expect("script should run");
        let icon = mutations.get("icon").expect("icon mutation should exist");

        assert_eq!(icon.stroke_color, Some(ColorToken::Blue));
        assert_eq!(icon.stroke_width, Some(3.0));
        assert_eq!(icon.fill_color, Some(ColorToken::Sky200));
        assert_eq!(icon.border_color, None);
        assert_eq!(icon.border_width, None);
        assert_eq!(icon.bg_color, None);
    }

    #[test]
    fn script_driver_records_standard_canvaskit_rect_and_image_commands() {
        let driver = ScriptDriver::from_source(
            r##"
            const CK = ctx.CanvasKit;
            const canvas = ctx.getCanvas();
            const fill = new CK.Paint();
            fill.setStyle(CK.PaintStyle.Fill);
            fill.setColor(CK.Color(255, 0, 0, 1));

            const image = ctx.getImage("hero");
            canvas
                .drawRect(CK.XYWHRect(0, 0, 40, 20), fill)
                .drawImageRect(
                    image,
                    CK.XYWHRect(0, 0, 1, 1),
                    CK.XYWHRect(10, 10, 80, 60),
                );
        "##,
        )
        .expect("script should compile");

        let mutations = driver
            .run(0, 1, 0, 1, Some("card"))
            .expect("script should run");
        let canvas = mutations
            .get_canvas("card")
            .expect("canvas mutation should exist");

        assert_eq!(
            canvas.commands[0],
            CanvasCommand::SetAntiAlias { enabled: true }
        );
    }
}
