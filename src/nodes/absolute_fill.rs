use std::any::Any;

use skia_safe::{Canvas, Paint, Rect};

use crate::{
    FrameCtx, Node, ViewNode,
    style::{
        AlignItems, ColorToken, ComputedTextStyle, FlexDirection, JustifyContent, NodeStyle,
        impl_node_style_api, resolve_text_style,
    },
};

pub struct AbsoluteFill {
    pub(crate) style: NodeStyle,
    children: Vec<Node>,
}

impl AbsoluteFill {
    pub fn new() -> Self {
        Self {
            style: NodeStyle {
                bg_color: Some(ColorToken::White),
                ..Default::default()
            },
            children: Vec::new(),
        }
    }

    pub fn child<T: Into<Node>>(mut self, child: T) -> Self {
        self.children.push(child.into());
        self
    }

    pub fn flex_direction_value(&self) -> FlexDirection {
        self.style.flex_direction.unwrap_or_default()
    }

    pub fn justify_content_value(&self) -> JustifyContent {
        self.style.justify_content.unwrap_or_default()
    }

    pub fn align_items_value(&self) -> AlignItems {
        self.style.align_items.unwrap_or_default()
    }

    pub fn gap_value(&self) -> f32 {
        self.style.gap.unwrap_or(0.0)
    }

    pub fn children_ref(&self) -> &[Node] {
        &self.children
    }

    pub fn background_color_value(&self) -> ColorToken {
        self.style.bg_color.unwrap_or(ColorToken::White)
    }

    pub fn style_ref(&self) -> &NodeStyle {
        &self.style
    }

    pub fn resolve_text_style(&self, inherited: &ComputedTextStyle) -> ComputedTextStyle {
        resolve_text_style(inherited, &self.style)
    }
}

impl_node_style_api!(AbsoluteFill);

impl ViewNode for AbsoluteFill {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn style_ref(&self) -> &NodeStyle {
        &self.style
    }

    fn draw(&self, ctx: &FrameCtx, canvas: &Canvas, bounds: Rect, computed_style: &ComputedTextStyle) {
        // Draw background
        let mut paint = Paint::default();
        paint.set_color(self.background_color_value().to_skia());
        paint.set_anti_alias(true);
        canvas.draw_rect(bounds, &paint);

        if self.children.is_empty() {
            return;
        }

        let next_style = self.resolve_text_style(computed_style);
        let direction = self.flex_direction_value();
        let justify = self.justify_content_value();
        let align = self.align_items_value();
        let gap = self.gap_value();

        let is_col = direction == FlexDirection::Col;
        let container_main = if is_col { bounds.height() } else { bounds.width() };
        let container_cross = if is_col { bounds.width() } else { bounds.height() };

        // Measure all children and calculate flex distribution
        let child_infos: Vec<_> = self.children.iter()
            .map(|child| {
                let intrinsic = child.intrinsic_size(ctx, &next_style);
                let (w, h) = intrinsic.unwrap_or((container_cross, container_main));
                let (main_size, cross_size) = if is_col { (h, w) } else { (w, h) };
                let flex_grow = child.style_ref().flex_grow.unwrap_or(0.0);
                (child, intrinsic, main_size, cross_size, flex_grow)
            })
            .collect();

        // Calculate total fixed size and flex grow sum
        let total_fixed: f32 = child_infos.iter().map(|(_, _, main, _, fg)| {
            if *fg > 0.0 { 0.0 } else { *main }
        }).sum();
        let total_flex_grow: f32 = child_infos.iter().map(|(_, _, _, _, fg)| *fg).sum();

        // Calculate gap total
        let gap_count = if child_infos.len() > 1 { child_infos.len() - 1 } else { 0 };
        let total_gap = gap * gap_count as f32;

        // Calculate remaining space for flex items
        let remaining = (container_main - total_fixed - total_gap).max(0.0);

        // Calculate positions
        let flex_unit = if total_flex_grow > 0.0 { remaining / total_flex_grow } else { 0.0 };

        // Calculate start offset based on justify content
        let total_used = total_fixed + total_gap + remaining;
        let extra_space = container_main - total_used;

        let start_offset = match justify {
            JustifyContent::Start => 0.0,
            JustifyContent::Center => extra_space / 2.0,
            JustifyContent::End => extra_space,
            JustifyContent::Between if child_infos.len() > 1 => 0.0,
            JustifyContent::Around => extra_space / (child_infos.len() as f32 * 2.0),
            JustifyContent::Evenly => extra_space / (child_infos.len() as f32 + 1.0),
            _ => 0.0,
        };

        // For space-between, calculate gap multiplier
        let between_gap = if justify == JustifyContent::Between && child_infos.len() > 1 {
            extra_space / (child_infos.len() - 1) as f32
        } else {
            0.0
        };

        let mut current_pos = start_offset;

        for (idx, (child, _intrinsic, base_main, base_cross, flex_grow)) in child_infos.iter().enumerate() {
            // Calculate main size (with flex grow)
            let main_size = if *flex_grow > 0.0 {
                base_main + flex_unit * flex_grow
            } else {
                *base_main
            };

            // Calculate cross size (with align-items: stretch)
            let cross_size = match align {
                AlignItems::Stretch => container_cross,
                _ => *base_cross,
            };

            // Calculate cross axis position
            let cross_offset = match align {
                AlignItems::Start | AlignItems::Stretch => 0.0,
                AlignItems::Center => (container_cross - cross_size) / 2.0,
                AlignItems::End => container_cross - cross_size,
            };

            // Build child bounds
            let (x, y, w, h) = if is_col {
                (bounds.left + cross_offset, bounds.top + current_pos, cross_size, main_size)
            } else {
                (bounds.left + current_pos, bounds.top + cross_offset, main_size, cross_size)
            };

            let child_bounds = Rect::from_xywh(x, y, w, h);
            child.draw(ctx, canvas, child_bounds, &next_style);

            // Advance position
            current_pos += main_size + gap;
            if justify == JustifyContent::Between && idx < child_infos.len() - 1 {
                current_pos += between_gap;
            }
            if justify == JustifyContent::Around {
                current_pos += extra_space / (child_infos.len() as f32 * 2.0) * 2.0;
            }
            if justify == JustifyContent::Evenly {
                current_pos += extra_space / (child_infos.len() as f32 + 1.0);
            }
        }
    }
}
