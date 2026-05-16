use skia_safe::{
    canvas::{SaveLayerRec, SrcRectConstraint},
    color_filters, gradient_shader, image_filters, images,
    BlurStyle as SkiaBlurStyle, Canvas, Color, ColorFilter, Data, Font, Image as SkiaImage,
    ImageInfo, MaskFilter, Matrix, Paint, PaintStyle as SkiaPaintStyle, Path as SkiaPath,
    PathBuilder, PathEffect, PathFillType, Picture, PictureRecorder, Point as SkiaPoint,
    RRect as SkiaRRect, Rect as SkiaRect, RuntimeEffect, Shader, TileMode as SkiaTileMode,
};

use opencat_core::canvas::{
    BlendMode, BlurStyle, Canvas2D, ClipOp, ColorFilterSpec, FillSpec, FillType, FontEdging,
    GlyphRunSpec, ImageFilterSpec, MaskFilterSpec, PaintSpec, PaintStyle, PathEffectSpec, PointMode,
    Rect, RRect, RuntimeEffectChild, ShaderSpec, StrokeCap, StrokeJoin, TileMode,
};

pub struct SkiaCanvas2D {
    canvas: *const Canvas,
    fill_paint: Paint,
    stroke_paint: Paint,
}

// SAFETY: Skia Canvas uses interior mutability; rendering is single-threaded.
unsafe impl Send for SkiaCanvas2D {}
unsafe impl Sync for SkiaCanvas2D {}

impl SkiaCanvas2D {
    pub fn new(canvas: &Canvas) -> Self {
        Self {
            canvas: canvas as *const Canvas,
            fill_paint: Paint::default(),
            stroke_paint: Paint::default(),
        }
    }

    fn canvas_ref(&self) -> &Canvas {
        // SAFETY: The pointer is valid as long as the underlying surface/canvas
        // outlives this SkiaCanvas2D, which is guaranteed by the caller.
        unsafe { &*self.canvas }
    }
}

impl Canvas2D for SkiaCanvas2D {
    type Path = SkiaPath;
    type Image = SkiaImage;
    type Picture = Picture;
    type RuntimeEffect = RuntimeEffect;

    fn save(&mut self) -> i32 {
        self.canvas_ref().save() as i32
    }

    fn save_layer(&mut self, bounds: Option<Rect>, alpha: f32) {
        let alpha_u8 = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
        let mut paint = Paint::default();
        paint.set_alpha(alpha_u8);
        let skia_bounds = bounds.as_ref().map(|b| rect_to_skia(b));
        let mut rec = SaveLayerRec::default().paint(&paint);
        if let Some(ref b) = skia_bounds {
            rec = rec.bounds(b);
        }
        self.canvas_ref().save_layer(&rec);
    }

    fn save_layer_with(&mut self, bounds: Option<Rect>, paint: &PaintSpec) {
        let mut skia_paint = Paint::default();
        apply_spec(&mut skia_paint, paint, paint.style);
        let skia_bounds = bounds.as_ref().map(|b| rect_to_skia(b));
        let mut rec = SaveLayerRec::default().paint(&skia_paint);
        if let Some(ref b) = skia_bounds {
            rec = rec.bounds(b);
        }
        self.canvas_ref().save_layer(&rec);
    }

    fn restore(&mut self) {
        self.canvas_ref().restore();
    }

    fn restore_to_count(&mut self, count: i32) {
        self.canvas_ref().restore_to_count(count as usize);
    }

    fn save_count(&self) -> i32 {
        self.canvas_ref().save_count() as i32
    }

    fn translate(&mut self, dx: f32, dy: f32) {
        self.canvas_ref().translate((dx, dy));
    }

    fn scale(&mut self, sx: f32, sy: f32) {
        self.canvas_ref().scale((sx, sy));
    }

    fn rotate(&mut self, degrees: f32, cx: f32, cy: f32) {
        self.canvas_ref().rotate(degrees, Some(SkiaPoint::new(cx, cy)));
    }

    fn skew(&mut self, sx: f32, sy: f32) {
        self.canvas_ref().skew((sx, sy));
    }

    fn concat(&mut self, matrix: &[f32; 9]) {
        let m = Matrix::new_all(
            matrix[0], matrix[1], matrix[2], matrix[3], matrix[4], matrix[5], matrix[6],
            matrix[7], matrix[8],
        );
        self.canvas_ref().concat(&m);
    }

    fn clip_rect(&mut self, rect: &Rect, op: ClipOp, anti_alias: bool) {
        self.canvas_ref()
            .clip_rect(rect_to_skia(rect), convert_clip_op(op), anti_alias);
    }

    fn clip_rrect(&mut self, rrect: &RRect, op: ClipOp, anti_alias: bool) {
        self.canvas_ref()
            .clip_rrect(rrect_to_skia(rrect), convert_clip_op(op), anti_alias);
    }

    fn clip_path(&mut self, path: &Self::Path, op: ClipOp, anti_alias: bool) {
        self.canvas_ref()
            .clip_path(path, convert_clip_op(op), anti_alias);
    }

    fn clear(&mut self, color: [f32; 4]) {
        self.canvas_ref().clear(float4_to_color(color));
    }

    fn draw_paint(&mut self, paint: &PaintSpec) {
        match paint.style {
            PaintStyle::Fill => {
                apply_spec(&mut self.fill_paint, paint, PaintStyle::Fill);
                self.canvas_ref().draw_paint(&self.fill_paint);
            }
            PaintStyle::Stroke => {
                apply_spec(&mut self.stroke_paint, paint, PaintStyle::Stroke);
                self.canvas_ref().draw_paint(&self.stroke_paint);
            }
        }
    }

    fn draw_rect(&mut self, rect: &Rect, paint: &PaintSpec) {
        let r = rect_to_skia(rect);
        match paint.style {
            PaintStyle::Fill => {
                apply_spec(&mut self.fill_paint, paint, PaintStyle::Fill);
                self.canvas_ref().draw_rect(r, &self.fill_paint);
            }
            PaintStyle::Stroke => {
                apply_spec(&mut self.stroke_paint, paint, PaintStyle::Stroke);
                self.canvas_ref().draw_rect(r, &self.stroke_paint);
            }
        }
    }

    fn draw_rrect(&mut self, rrect: &RRect, paint: &PaintSpec) {
        let rr = rrect_to_skia(rrect);
        match paint.style {
            PaintStyle::Fill => {
                apply_spec(&mut self.fill_paint, paint, PaintStyle::Fill);
                self.canvas_ref().draw_rrect(rr, &self.fill_paint);
            }
            PaintStyle::Stroke => {
                apply_spec(&mut self.stroke_paint, paint, PaintStyle::Stroke);
                self.canvas_ref().draw_rrect(rr, &self.stroke_paint);
            }
        }
    }

    fn draw_drrect(&mut self, outer: &RRect, inner: &RRect, paint: &PaintSpec) {
        let o = rrect_to_skia(outer);
        let i = rrect_to_skia(inner);
        match paint.style {
            PaintStyle::Fill => {
                apply_spec(&mut self.fill_paint, paint, PaintStyle::Fill);
                self.canvas_ref().draw_drrect(o, i, &self.fill_paint);
            }
            PaintStyle::Stroke => {
                apply_spec(&mut self.stroke_paint, paint, PaintStyle::Stroke);
                self.canvas_ref().draw_drrect(o, i, &self.stroke_paint);
            }
        }
    }

    fn draw_oval(&mut self, oval: &Rect, paint: &PaintSpec) {
        let o = rect_to_skia(oval);
        match paint.style {
            PaintStyle::Fill => {
                apply_spec(&mut self.fill_paint, paint, PaintStyle::Fill);
                self.canvas_ref().draw_oval(o, &self.fill_paint);
            }
            PaintStyle::Stroke => {
                apply_spec(&mut self.stroke_paint, paint, PaintStyle::Stroke);
                self.canvas_ref().draw_oval(o, &self.stroke_paint);
            }
        }
    }

    fn draw_circle(&mut self, cx: f32, cy: f32, radius: f32, paint: &PaintSpec) {
        match paint.style {
            PaintStyle::Fill => {
                apply_spec(&mut self.fill_paint, paint, PaintStyle::Fill);
                self.canvas_ref().draw_circle((cx, cy), radius, &self.fill_paint);
            }
            PaintStyle::Stroke => {
                apply_spec(&mut self.stroke_paint, paint, PaintStyle::Stroke);
                self.canvas_ref()
                    .draw_circle((cx, cy), radius, &self.stroke_paint);
            }
        }
    }

    fn draw_arc(
        &mut self,
        oval: &Rect,
        start: f32,
        sweep: f32,
        use_center: bool,
        paint: &PaintSpec,
    ) {
        let o = rect_to_skia(oval);
        match paint.style {
            PaintStyle::Fill => {
                apply_spec(&mut self.fill_paint, paint, PaintStyle::Fill);
                self.canvas_ref()
                    .draw_arc(o, start, sweep, use_center, &self.fill_paint);
            }
            PaintStyle::Stroke => {
                apply_spec(&mut self.stroke_paint, paint, PaintStyle::Stroke);
                self.canvas_ref()
                    .draw_arc(o, start, sweep, use_center, &self.stroke_paint);
            }
        }
    }

    fn draw_line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, paint: &PaintSpec) {
        match paint.style {
            PaintStyle::Fill => {
                apply_spec(&mut self.fill_paint, paint, PaintStyle::Fill);
                self.canvas_ref()
                    .draw_line((x0, y0), (x1, y1), &self.fill_paint);
            }
            PaintStyle::Stroke => {
                apply_spec(&mut self.stroke_paint, paint, PaintStyle::Stroke);
                self.canvas_ref()
                    .draw_line((x0, y0), (x1, y1), &self.stroke_paint);
            }
        }
    }

    fn draw_points(&mut self, mode: PointMode, pts: &[f32], paint: &PaintSpec) {
        let skia_points: Vec<SkiaPoint> = pts
            .chunks_exact(2)
            .map(|chunk| SkiaPoint::new(chunk[0], chunk[1]))
            .collect();
        match paint.style {
            PaintStyle::Fill => {
                apply_spec(&mut self.fill_paint, paint, PaintStyle::Fill);
                self.canvas_ref()
                    .draw_points(convert_point_mode(mode), &skia_points, &self.fill_paint);
            }
            PaintStyle::Stroke => {
                apply_spec(&mut self.stroke_paint, paint, PaintStyle::Stroke);
                self.canvas_ref()
                    .draw_points(convert_point_mode(mode), &skia_points, &self.stroke_paint);
            }
        }
    }

    fn draw_path(&mut self, path: &Self::Path, paint: &PaintSpec) {
        match paint.style {
            PaintStyle::Fill => {
                apply_spec(&mut self.fill_paint, paint, PaintStyle::Fill);
                self.canvas_ref().draw_path(path, &self.fill_paint);
            }
            PaintStyle::Stroke => {
                apply_spec(&mut self.stroke_paint, paint, PaintStyle::Stroke);
                self.canvas_ref().draw_path(path, &self.stroke_paint);
            }
        }
    }

    fn draw_image(
        &mut self,
        image: &Self::Image,
        x: f32,
        y: f32,
        paint: Option<&PaintSpec>,
    ) {
        if let Some(spec) = paint {
            apply_spec(&mut self.fill_paint, spec, PaintStyle::Fill);
            self.canvas_ref()
                .draw_image(image, (x, y), Some(&self.fill_paint));
        } else {
            self.canvas_ref()
                .draw_image(image, (x, y), None::<&Paint>);
        }
    }

    fn draw_image_rect(
        &mut self,
        image: &Self::Image,
        src: Option<&Rect>,
        dst: &Rect,
        paint: Option<&PaintSpec>,
    ) {
        let skia_dst = rect_to_skia(dst);
        if let Some(spec) = paint {
            apply_spec(&mut self.fill_paint, spec, PaintStyle::Fill);
            if let Some(src_rect) = src {
                let skia_src = rect_to_skia(src_rect);
                self.canvas_ref().draw_image_rect(
                    image,
                    Some((&skia_src, SrcRectConstraint::Strict)),
                    skia_dst,
                    &self.fill_paint,
                );
            } else {
                self.canvas_ref()
                    .draw_image_rect(image, None, skia_dst, &self.fill_paint);
            }
        } else if let Some(src_rect) = src {
            let skia_src = rect_to_skia(src_rect);
            self.canvas_ref().draw_image_rect(
                image,
                Some((&skia_src, SrcRectConstraint::Strict)),
                skia_dst,
                &Paint::default(),
            );
        } else {
            self.canvas_ref()
                .draw_image_rect(image, None, skia_dst, &Paint::default());
        }
    }

    fn draw_simple_text(
        &mut self,
        text: &str,
        x: f32,
        y: f32,
        font_size: f32,
        paint: &PaintSpec,
    ) {
        let mut font = Font::default();
        font.set_size(font_size);
        match paint.style {
            PaintStyle::Fill => {
                apply_spec(&mut self.fill_paint, paint, PaintStyle::Fill);
                self.canvas_ref()
                    .draw_str(text, (x, y), &font, &self.fill_paint);
            }
            PaintStyle::Stroke => {
                apply_spec(&mut self.stroke_paint, paint, PaintStyle::Stroke);
                self.canvas_ref()
                    .draw_str(text, (x, y), &font, &self.stroke_paint);
            }
        }
    }

    fn draw_glyph_run(&mut self, run: &GlyphRunSpec, paint: &PaintSpec) {
        debug_assert_eq!(run.positions.len() % 2, 0, "glyph positions must be even");
        let mut font = Font::default();
        font.set_size(run.font_size);
        font.set_scale_x(run.font_scale_x);
        font.set_skew_x(run.font_skew_x);
        font.set_subpixel(run.subpixel);
        font.set_edging(convert_font_edging(run.edging));

        let positions: Vec<SkiaPoint> = run
            .positions
            .chunks_exact(2)
            .map(|chunk| SkiaPoint::new(chunk[0], chunk[1]))
            .collect();

        match paint.style {
            PaintStyle::Fill => {
                apply_spec(&mut self.fill_paint, paint, PaintStyle::Fill);
                self.canvas_ref().draw_glyphs_at(
                    run.glyph_ids,
                    &*positions,
                    SkiaPoint::new(0.0, 0.0),
                    &font,
                    &self.fill_paint,
                );
            }
            PaintStyle::Stroke => {
                apply_spec(&mut self.stroke_paint, paint, PaintStyle::Stroke);
                self.canvas_ref().draw_glyphs_at(
                    run.glyph_ids,
                    &*positions,
                    SkiaPoint::new(0.0, 0.0),
                    &font,
                    &self.stroke_paint,
                );
            }
        }
    }

    fn make_picture<R>(&mut self, bounds: &Rect, record: R) -> Self::Picture
    where
        R: FnOnce(&mut Self),
        Self: Sized,
    {
        let skia_bounds = rect_to_skia(bounds);
        let mut recorder = PictureRecorder::new();
        let recording_canvas = recorder.begin_recording(skia_bounds, false);
        let canvas_ptr: *const Canvas = recording_canvas;
        // SAFETY: canvas points to valid Canvas owned by recorder, which outlives temp
        // because temp is dropped before finish_recording_as_picture
        let canvas_ref: &Canvas = unsafe { &*canvas_ptr };
        let mut temp = SkiaCanvas2D::new(canvas_ref);
        record(&mut temp);
        drop(temp);
        recorder
            .finish_recording_as_picture(None)
            .expect("failed to finish picture recording")
    }

    fn draw_picture(
        &mut self,
        picture: &Self::Picture,
        matrix: Option<&[f32; 9]>,
        paint: Option<&PaintSpec>,
    ) {
        let m = matrix.map(|m| {
            Matrix::new_all(
                m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8],
            )
        });
        if let Some(spec) = paint {
            apply_spec(&mut self.fill_paint, spec, PaintStyle::Fill);
            self.canvas_ref()
                .draw_picture(picture, m.as_ref(), Some(&self.fill_paint));
        } else {
            self.canvas_ref()
                .draw_picture(picture, m.as_ref(), None::<&Paint>);
        }
    }

    fn draw_runtime_effect(
        &mut self,
        effect: &Self::RuntimeEffect,
        uniforms: &[u8],
        children: &[RuntimeEffectChild<'_, Self>],
        dst: &Rect,
    ) {
        let child_shaders: Vec<Shader> = children
            .iter()
            .filter_map(build_runtime_effect_child)
            .collect();
        let child_ptrs: Vec<skia_safe::runtime_effect::ChildPtr> = child_shaders
            .iter()
            .map(|s| skia_safe::runtime_effect::ChildPtr::from(s.clone()))
            .collect();

        let data = Data::new_copy(uniforms);
        if let Some(shader) = effect.make_shader(data, &child_ptrs, None) {
            let mut paint = Paint::default();
            paint.set_shader(shader);
            let r = rect_to_skia(dst);
            self.canvas_ref().draw_rect(r, &paint);
        }
    }

    fn make_runtime_effect(&self, sksl: &str) -> Result<Self::RuntimeEffect, String> {
        RuntimeEffect::make_for_shader(sksl, None)
    }

    fn make_path_from_verbs(
        &self,
        verbs: &[u8],
        points: &[f32],
        fill_type: FillType,
    ) -> Self::Path {
        let skia_fill = match fill_type {
            FillType::Winding => PathFillType::Winding,
            FillType::EvenOdd => PathFillType::EvenOdd,
        };
        let mut builder = PathBuilder::new_with_fill_type(skia_fill);
        let mut pi = 0;
        for &verb in verbs {
            match verb {
                0 => {
                    builder.move_to((points[pi], points[pi + 1]));
                    pi += 2;
                }
                1 => {
                    builder.line_to((points[pi], points[pi + 1]));
                    pi += 2;
                }
                2 => {
                    builder.quad_to(
                        (points[pi], points[pi + 1]),
                        (points[pi + 2], points[pi + 3]),
                    );
                    pi += 4;
                }
                3 => {
                    builder.cubic_to(
                        (points[pi], points[pi + 1]),
                        (points[pi + 2], points[pi + 3]),
                        (points[pi + 4], points[pi + 5]),
                    );
                    pi += 6;
                }
                4 => {
                    builder.close();
                }
                _ => {}
            }
        }
        builder.snapshot()
    }

    fn make_path_from_svg(&self, svg_path_data: &str) -> Option<Self::Path> {
        SkiaPath::from_svg(svg_path_data)
    }

    fn make_image_from_rgba(&self, bytes: &[u8], width: u32, height: u32) -> Self::Image {
        let info = ImageInfo::new(
            (width as i32, height as i32),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Unpremul,
            None,
        );
        let data = Data::new_copy(bytes);
        images::raster_from_data(&info, data, width as usize * 4)
            .expect("failed to create image from RGBA bytes")
    }

    fn make_image_from_encoded(&self, bytes: &[u8]) -> Option<Self::Image> {
        let data = Data::new_copy(bytes);
        SkiaImage::from_encoded(data)
    }

    fn render_to_image<R>(&mut self, width: u32, height: u32, draw: R) -> Self::Image
    where
        R: FnOnce(&mut Self),
        Self: Sized,
    {
        let info = ImageInfo::new(
            (width as i32, height as i32),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Premul,
            None,
        );
        let mut surface = skia_safe::surfaces::raster(&info, None, None)
            .expect("failed to create offscreen surface");
        let canvas_ptr: *const Canvas = surface.canvas();
        let canvas_ref: &Canvas = unsafe { &*canvas_ptr };
        let mut temp = SkiaCanvas2D::new(canvas_ref);
        draw(&mut temp);
        drop(temp);
        surface.image_snapshot()
    }
}

fn rect_to_skia(r: &Rect) -> SkiaRect {
    SkiaRect::from_xywh(r.x0 as f32, r.y0 as f32, r.width() as f32, r.height() as f32)
}

fn rrect_to_skia(r: &RRect) -> SkiaRRect {
    let rect = rect_to_skia(&r.rect());
    let radii = r.radii();
    // Precision loss from f64 to f32 cast is acceptable for rendering
    let points = [
        SkiaPoint::new(radii.top_left as f32, radii.top_left as f32),
        SkiaPoint::new(radii.top_right as f32, radii.top_right as f32),
        SkiaPoint::new(radii.bottom_right as f32, radii.bottom_right as f32),
        SkiaPoint::new(radii.bottom_left as f32, radii.bottom_left as f32),
    ];
    SkiaRRect::new_rect_radii(rect, &points)
}

fn float4_to_color(c: [f32; 4]) -> Color {
    Color::from_argb(
        (c[3].clamp(0.0, 1.0) * 255.0) as u8,
        (c[0].clamp(0.0, 1.0) * 255.0) as u8,
        (c[1].clamp(0.0, 1.0) * 255.0) as u8,
        (c[2].clamp(0.0, 1.0) * 255.0) as u8,
    )
}

fn convert_clip_op(op: ClipOp) -> skia_safe::ClipOp {
    match op {
        ClipOp::Intersect => skia_safe::ClipOp::Intersect,
        ClipOp::Difference => skia_safe::ClipOp::Difference,
    }
}

fn convert_stroke_cap(c: StrokeCap) -> skia_safe::paint::Cap {
    match c {
        StrokeCap::Butt => skia_safe::paint::Cap::Butt,
        StrokeCap::Round => skia_safe::paint::Cap::Round,
        StrokeCap::Square => skia_safe::paint::Cap::Square,
    }
}

fn convert_stroke_join(j: StrokeJoin) -> skia_safe::paint::Join {
    match j {
        StrokeJoin::Miter => skia_safe::paint::Join::Miter,
        StrokeJoin::Round => skia_safe::paint::Join::Round,
        StrokeJoin::Bevel => skia_safe::paint::Join::Bevel,
    }
}

fn convert_blur_style(s: BlurStyle) -> SkiaBlurStyle {
    match s {
        BlurStyle::Normal => SkiaBlurStyle::Normal,
        BlurStyle::Inner => SkiaBlurStyle::Inner,
        BlurStyle::Solid => SkiaBlurStyle::Solid,
        BlurStyle::Outer => SkiaBlurStyle::Outer,
    }
}

fn convert_point_mode(m: PointMode) -> skia_safe::canvas::PointMode {
    match m {
        PointMode::Points => skia_safe::canvas::PointMode::Points,
        PointMode::Lines => skia_safe::canvas::PointMode::Lines,
        PointMode::Polygon => skia_safe::canvas::PointMode::Polygon,
    }
}

fn convert_tile_mode(t: TileMode) -> SkiaTileMode {
    match t {
        TileMode::Clamp => SkiaTileMode::Clamp,
        TileMode::Repeat => SkiaTileMode::Repeat,
        TileMode::Mirror => SkiaTileMode::Mirror,
        TileMode::Decal => SkiaTileMode::Decal,
    }
}

fn convert_font_edging(e: FontEdging) -> skia_safe::font::Edging {
    match e {
        FontEdging::Alias => skia_safe::font::Edging::Alias,
        FontEdging::AntiAlias => skia_safe::font::Edging::AntiAlias,
        FontEdging::SubpixelAntiAlias => skia_safe::font::Edging::SubpixelAntiAlias,
    }
}

fn convert_blend_mode(m: BlendMode) -> skia_safe::BlendMode {
    match m {
        BlendMode::Clear => skia_safe::BlendMode::Clear,
        BlendMode::Src => skia_safe::BlendMode::Src,
        BlendMode::Dst => skia_safe::BlendMode::Dst,
        BlendMode::SrcOver => skia_safe::BlendMode::SrcOver,
        BlendMode::DstOver => skia_safe::BlendMode::DstOver,
        BlendMode::SrcIn => skia_safe::BlendMode::SrcIn,
        BlendMode::DstIn => skia_safe::BlendMode::DstIn,
        BlendMode::SrcOut => skia_safe::BlendMode::SrcOut,
        BlendMode::DstOut => skia_safe::BlendMode::DstOut,
        BlendMode::SrcATop => skia_safe::BlendMode::SrcATop,
        BlendMode::DstATop => skia_safe::BlendMode::DstATop,
        BlendMode::Xor => skia_safe::BlendMode::Xor,
        BlendMode::Plus => skia_safe::BlendMode::Plus,
        BlendMode::Modulate => skia_safe::BlendMode::Modulate,
        BlendMode::Screen => skia_safe::BlendMode::Screen,
        BlendMode::Overlay => skia_safe::BlendMode::Overlay,
        BlendMode::Darken => skia_safe::BlendMode::Darken,
        BlendMode::Lighten => skia_safe::BlendMode::Lighten,
        BlendMode::ColorDodge => skia_safe::BlendMode::ColorDodge,
        BlendMode::ColorBurn => skia_safe::BlendMode::ColorBurn,
        BlendMode::HardLight => skia_safe::BlendMode::HardLight,
        BlendMode::SoftLight => skia_safe::BlendMode::SoftLight,
        BlendMode::Difference => skia_safe::BlendMode::Difference,
        BlendMode::Exclusion => skia_safe::BlendMode::Exclusion,
        BlendMode::Multiply => skia_safe::BlendMode::Multiply,
        BlendMode::Hue => skia_safe::BlendMode::Hue,
        BlendMode::Saturation => skia_safe::BlendMode::Saturation,
        BlendMode::Color => skia_safe::BlendMode::Color,
        BlendMode::Luminosity => skia_safe::BlendMode::Luminosity,
    }
}

fn apply_spec(paint: &mut Paint, spec: &PaintSpec, style: PaintStyle) {
    *paint = Paint::default();
    paint.set_style(match style {
        PaintStyle::Fill => SkiaPaintStyle::Fill,
        PaintStyle::Stroke => SkiaPaintStyle::Stroke,
    });
    paint.set_anti_alias(spec.anti_alias);

    match &spec.fill {
        FillSpec::Solid(color) => {
            paint.set_shader(None);
            paint.set_color(float4_to_color(*color));
        }
        FillSpec::Shader(shader_spec) => {
            if let Some(shader) = build_skia_shader(shader_spec) {
                paint.set_shader(shader);
            }
        }
    }

    if let Some(stroke) = &spec.stroke {
        paint.set_stroke_width(stroke.width);
        paint.set_stroke_cap(convert_stroke_cap(stroke.cap));
        paint.set_stroke_join(convert_stroke_join(stroke.join));
        paint.set_stroke_miter(stroke.miter_limit);
    }

    paint.set_blend_mode(convert_blend_mode(spec.blend_mode));

    if let Some(filter) = &spec.image_filter {
        if let Some(skia_filter) = build_skia_image_filter(filter) {
            paint.set_image_filter(skia_filter);
        }
    }

    if let Some(cf) = &spec.color_filter {
        if let Some(skia_cf) = build_color_filter(cf) {
            paint.set_color_filter(skia_cf);
        }
    }

    if let Some(mask) = &spec.mask_filter {
        match mask {
            MaskFilterSpec::Blur {
                sigma,
                style,
                respect_ctm,
            } => {
                if let Some(mask_filter) =
                    MaskFilter::blur(convert_blur_style(*style), *sigma, *respect_ctm)
                {
                    paint.set_mask_filter(mask_filter);
                }
            }
        }
    }

    if let Some(effect) = &spec.path_effect {
        match effect {
            PathEffectSpec::Dash { intervals, phase } => {
                if let Some(pe) = PathEffect::dash(intervals, *phase) {
                    paint.set_path_effect(pe);
                }
            }
        }
    }
}

fn build_skia_shader(spec: &ShaderSpec) -> Option<Shader> {
    match spec {
        ShaderSpec::LinearGradient {
            from,
            to,
            stops,
            colors,
            tile_mode,
        } => {
            let skia_colors: Vec<Color> = colors
                .iter()
                .map(|c| float4_to_color(*c))
                .collect();
            gradient_shader::linear(
                ((from[0], from[1]), (to[0], to[1])),
                skia_colors.as_slice(),
                Some(stops.as_slice()),
                convert_tile_mode(*tile_mode),
                None,
                None,
            )
        }
        ShaderSpec::RadialGradient {
            center,
            radius,
            stops,
            colors,
            tile_mode,
        } => {
            let skia_colors: Vec<Color> = colors
                .iter()
                .map(|c| float4_to_color(*c))
                .collect();
            gradient_shader::radial(
                (center[0], center[1]),
                *radius,
                skia_colors.as_slice(),
                Some(stops.as_slice()),
                convert_tile_mode(*tile_mode),
                None,
                None,
            )
        }
    }
}

fn build_skia_image_filter(spec: &ImageFilterSpec) -> Option<skia_safe::ImageFilter> {
    match spec {
        ImageFilterSpec::Blur {
            sigma_x,
            sigma_y,
            crop_rect,
        } => {
            let crop = crop_rect.as_ref().map(|r| rect_to_skia(r));
            image_filters::blur(
                (*sigma_x, *sigma_y),
                SkiaTileMode::Clamp,
                None,
                crop.map(image_filters::CropRect::from),
            )
        }
        ImageFilterSpec::DropShadow {
            dx,
            dy,
            sigma_x,
            sigma_y,
            color,
        } => {
            let c = float4_to_color(*color);
            image_filters::drop_shadow_only(
                (*dx, *dy),
                (*sigma_x, *sigma_y),
                c,
                None::<skia_safe::ColorSpace>,
                None::<skia_safe::ImageFilter>,
                None::<skia_safe::image_filters::CropRect>,
            )
        }
        ImageFilterSpec::ColorFilter(cf) => {
            if let Some(skia_cf) = build_color_filter(cf) {
                image_filters::color_filter(
                    skia_cf,
                    None::<skia_safe::ImageFilter>,
                    None::<skia_safe::image_filters::CropRect>,
                )
            } else {
                None
            }
        }
        ImageFilterSpec::Compose(outer, inner) => {
            let out = build_skia_image_filter(outer)?;
            let inn = build_skia_image_filter(inner)?;
            image_filters::compose(out, inn)
        }
    }
}

fn build_color_filter(spec: &ColorFilterSpec) -> Option<ColorFilter> {
    match spec {
        ColorFilterSpec::Matrix(matrix) => {
            Some(color_filters::matrix_row_major(matrix, None))
        }
        ColorFilterSpec::BlendColor { color, mode } => {
            color_filters::blend(float4_to_color(*color), convert_blend_mode(*mode))
        }
        ColorFilterSpec::LinearToSrgbGamma => {
            Some(color_filters::linear_to_srgb_gamma())
        }
        ColorFilterSpec::SrgbToLinearGamma => {
            Some(color_filters::srgb_to_linear_gamma())
        }
    }
}

fn build_runtime_effect_child(child: &RuntimeEffectChild<SkiaCanvas2D>) -> Option<Shader> {
    match child {
        RuntimeEffectChild::Texture(img) => img.to_shader(
            (SkiaTileMode::Clamp, SkiaTileMode::Clamp),
            skia_safe::SamplingOptions::default(),
            None,
        ),
        RuntimeEffectChild::Shader(spec) => build_skia_shader(spec),
    }
}
