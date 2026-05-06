// ── Native Bridge ──
// Thin wrapper that routes window.__record_*, window.__canvas_*,
// window.__animate_*, window.__text_source_* to the wasm-side
// WebMutationRecorder. Replaces the previous TS-side accumulation logic.

import init, { WebMutationRecorder } from '../wasm/opencat_web.js';

export class NativeBridge {
  private rec!: WebMutationRecorder;
  private currentFrame = 0;

  async init(wasmUrl?: string): Promise<void> {
    await init(wasmUrl);
    this.rec = new WebMutationRecorder();
  }

  setFrameCtx(frame: number, _totalFrames: number, _sceneFrames: number): void {
    this.currentFrame = frame;
    this.rec.reset_for_frame(frame);
  }

  reset(): void {
    this.rec.reset_for_frame(this.currentFrame);
  }

  collectJson(): string {
    return this.rec.snapshot_mutations_json();
  }

  injectGlobals(): void {
    const r = this.rec;
    const w = window as any;

    w.__record_opacity = (id: string, v: number) => r.record_opacity(id, v);
    w.__record_translate_x = (id: string, v: number) => r.record_translate_x(id, v);
    w.__record_translate_y = (id: string, v: number) => r.record_translate_y(id, v);
    w.__record_translate = (id: string, x: number, y: number) => r.record_translate(id, x, y);
    w.__record_scale = (id: string, v: number) => r.record_scale(id, v);
    w.__record_scale_x = (id: string, v: number) => r.record_scale_x(id, v);
    w.__record_scale_y = (id: string, v: number) => r.record_scale_y(id, v);
    w.__record_rotate = (id: string, v: number) => r.record_rotate(id, v);
    w.__record_skew_x = (id: string, v: number) => r.record_skew_x(id, v);
    w.__record_skew_y = (id: string, v: number) => r.record_skew_y(id, v);
    w.__record_skew = (id: string, x: number, y: number) => r.record_skew(id, x, y);
    w.__record_position = (id: string, v: string) => r.record_position(id, v);
    w.__record_left = (id: string, v: number) => r.record_left(id, v);
    w.__record_top = (id: string, v: number) => r.record_top(id, v);
    w.__record_right = (id: string, v: number) => r.record_right(id, v);
    w.__record_bottom = (id: string, v: number) => r.record_bottom(id, v);
    w.__record_width = (id: string, v: number) => r.record_width(id, v);
    w.__record_height = (id: string, v: number) => r.record_height(id, v);
    w.__record_padding = (id: string, v: number) => r.record_padding(id, v);
    w.__record_padding_x = (id: string, v: number) => r.record_padding_x(id, v);
    w.__record_padding_y = (id: string, v: number) => r.record_padding_y(id, v);
    w.__record_margin = (id: string, v: number) => r.record_margin(id, v);
    w.__record_margin_x = (id: string, v: number) => r.record_margin_x(id, v);
    w.__record_margin_y = (id: string, v: number) => r.record_margin_y(id, v);
    w.__record_flex_direction = (id: string, v: string) => r.record_flex_direction(id, v);
    w.__record_justify_content = (id: string, v: string) => r.record_justify_content(id, v);
    w.__record_align_items = (id: string, v: string) => r.record_align_items(id, v);
    w.__record_gap = (id: string, v: number) => r.record_gap(id, v);
    w.__record_flex_grow = (id: string, v: number) => r.record_flex_grow(id, v);
    w.__record_bg = (id: string, v: string) => r.record_bg_color(id, v);
    w.__record_border_radius = (id: string, v: number) => r.record_border_radius(id, v);
    w.__record_border_width = (id: string, v: number) => r.record_border_width(id, v);
    w.__record_border_top_width = (id: string, v: number) => r.record_border_top_width(id, v);
    w.__record_border_right_width = (id: string, v: number) => r.record_border_right_width(id, v);
    w.__record_border_bottom_width = (id: string, v: number) => r.record_border_bottom_width(id, v);
    w.__record_border_left_width = (id: string, v: number) => r.record_border_left_width(id, v);
    w.__record_border_style = (id: string, v: string) => r.record_border_style(id, v);
    w.__record_border_color = (id: string, v: string) => r.record_border_color(id, v);
    w.__record_stroke_width = (id: string, v: number) => r.record_stroke_width(id, v);
    w.__record_stroke_color = (id: string, v: string) => r.record_stroke_color(id, v);
    w.__record_fill_color = (id: string, v: string) => r.record_fill_color(id, v);
    w.__record_object_fit = (id: string, v: string) => r.record_object_fit(id, v);
    w.__record_text_color = (id: string, v: string) => r.record_text_color(id, v);
    w.__record_text_size = (id: string, v: number) => r.record_text_size(id, v);
    w.__record_font_weight = (id: string, v: number) => r.record_font_weight(id, v);
    w.__record_letter_spacing = (id: string, v: number) => r.record_letter_spacing(id, v);
    w.__record_text_align = (id: string, v: string) => r.record_text_align(id, v);
    w.__record_line_height = (id: string, v: number) => r.record_line_height(id, v);
    w.__record_shadow = (id: string, v: string) => r.record_box_shadow(id, v);
    w.__record_shadow_color = (id: string, v: string) => r.record_box_shadow_color(id, v);
    w.__record_inset_shadow = (id: string, v: string) => r.record_inset_shadow(id, v);
    w.__record_inset_shadow_color = (id: string, v: string) => r.record_inset_shadow_color(id, v);
    w.__record_drop_shadow = (id: string, v: string) => r.record_drop_shadow(id, v);
    w.__record_drop_shadow_color = (id: string, v: string) => r.record_drop_shadow_color(id, v);
    w.__record_text_content = (id: string, v: string) => r.record_text_content(id, v);
    w.__record_stroke_dasharray = (id: string, v: number) => r.record_stroke_dasharray(id, v);
    w.__record_stroke_dashoffset = (id: string, v: number) => r.record_stroke_dashoffset(id, v);
    w.__record_svg_path = (id: string, v: string) => r.record_svg_path(id, v);

    w.__text_source_get = (id: string) => r.get_text(id) || '';
    w.__text_source_set = (id: string, t: string) => r.set_text(id, t);

    w.__canvas_save = (id: string) => r.record_canvas_save(id);
    w.__canvas_restore = (id: string) => r.record_canvas_restore(id);
    w.__canvas_translate = (id: string, x: number, y: number) => r.record_canvas_translate(id, x, y);
    w.__canvas_scale = (id: string, x: number, y: number) => r.record_canvas_scale(id, x, y);
    w.__canvas_rotate = (id: string, deg: number) => r.record_canvas_rotate(id, deg);
    w.__canvas_clip_rect = (id: string, x: number, y: number, w_: number, h_: number, aa: boolean) =>
      r.record_canvas_clip_rect(id, x, y, w_, h_, aa);
    w.__canvas_fill_rect = (id: string, x: number, y: number, w_: number, h_: number) =>
      r.record_canvas_fill_rect(id, x, y, w_, h_, 'rgba(0,0,0,1)');
    w.__canvas_fill_rrect = (id: string, x: number, y: number, w_: number, h_: number, rad: number) =>
      r.record_canvas_fill_rrect(id, x, y, w_, h_, rad);
    w.__canvas_fill_circle = (id: string, cx: number, cy: number, rad: number) =>
      r.record_canvas_fill_circle(id, cx, cy, rad);
    w.__canvas_stroke_circle = (id: string, cx: number, cy: number, rad: number) =>
      r.record_canvas_stroke_circle(id, cx, cy, rad);
    w.__canvas_draw_line = (id: string, x0: number, y0: number, x1: number, y1: number) =>
      r.record_canvas_draw_line(id, x0, y0, x1, y1);
    w.__canvas_begin_path = (id: string) => r.record_canvas_begin_path(id);
    w.__canvas_move_to = (id: string, x: number, y: number) => r.record_canvas_move_to(id, x, y);
    w.__canvas_line_to = (id: string, x: number, y: number) => r.record_canvas_line_to(id, x, y);
    w.__canvas_quad_to = (id: string, cx: number, cy: number, x: number, y: number) =>
      r.record_canvas_quad_to(id, cx, cy, x, y);
    w.__canvas_cubic_to = (id: string, c1x: number, c1y: number, c2x: number, c2y: number,
                           x: number, y: number) =>
      r.record_canvas_cubic_to(id, c1x, c1y, c2x, c2y, x, y);
    w.__canvas_close_path = (id: string) => r.record_canvas_close_path(id);
    w.__canvas_fill_path = (id: string) => r.record_canvas_fill_path(id);
    w.__canvas_stroke_path = (id: string) => r.record_canvas_stroke_path(id);
    w.__canvas_set_fill_style = (id: string, c: string) => r.record_canvas_set_fill_style(id, c);
    w.__canvas_set_stroke_style = (id: string, c: string) => r.record_canvas_set_stroke_style(id, c);
    w.__canvas_set_line_width = (id: string, w_: number) => r.record_canvas_set_line_width(id, w_);
    w.__canvas_set_global_alpha = (id: string, a: number) => r.record_canvas_set_global_alpha(id, a);
    w.__canvas_clear = (id: string, c: string | null) => r.record_canvas_clear(id, c ?? 'rgba(0,0,0,0)');
    w.__canvas_draw_image = (id: string, asset: string, x: number, y: number,
                             w_: number, h_: number, alpha: number) =>
      r.record_canvas_draw_image(id, asset, x, y, w_, h_, alpha);
    w.__canvas_draw_image_simple = (id: string, asset: string, x: number, y: number, alpha: number) =>
      r.record_canvas_draw_image_simple(id, asset, x, y, alpha);
    w.__canvas_draw_text = (id: string, txt: string, x: number, y: number, fs: number, aa: boolean,
                            stroke: boolean, sw: number) =>
      r.record_canvas_draw_text(id, txt, x, y, fs, aa, stroke, sw, 'rgba(0,0,0,1)');

    w.__animate_create = (dur: number, del: number, clmp: number, ease: string,
                          rep: number, yoyo: number, rd: number) =>
      r.animate_create(this.currentFrame, dur, del, clmp !== 0, ease, rep, yoyo !== 0, rd);
    w.__animate_value = (h: number, _k: string, from: number, to: number) =>
      r.animate_value(this.currentFrame, h, from, to);
    w.__animate_color = (h: number, _k: string, from: string, to: string) =>
      r.animate_color(h, from, to);
    w.__animate_progress = (h: number) => r.animate_progress(h);
    w.__animate_settled = (h: number) => r.animate_settled(h);
    w.__animate_settle_frame = (h: number) => r.animate_settle_frame(h);
    w.__animate_dispose = (_h: number) => { /* no-op: states reset per frame */ };
    w.__flush_timelines = () => { /* no-op: animations compute on-demand */ };

    w.__morph_svg_create = (from: string, to: string, grid: number) =>
      r.morph_svg_create(from, to, grid);
    w.__morph_svg_sample = (h: number, t: number, tol: number) =>
      r.morph_svg_sample(h, t, tol);
    w.__morph_svg_dispose = (h: number) => r.morph_svg_dispose(h);

    w.__along_path_create = (svg: string) => r.along_path_create(svg);
    w.__along_path_length = (h: number) => r.along_path_length(h);
    w.__along_path_at = (h: number, t: number): number[] =>
      Array.from(r.along_path_at(h, t));
    w.__along_path_dispose = (h: number) => r.along_path_dispose(h);

    w.__util_random_seeded = (seed: number) => r.random_from_seed(seed);
    w.__easing_apply = (tag: string, t: number) => r.easing_apply(tag, t);

    w.__text_units_describe = (id: string, gran: string) => r.text_units_describe(id, gran);

    w.__record_text_unit_override = (id: string, gran: string, idx: number, vals: any) =>
      r.record_text_unit_override(
        id, gran, idx,
        vals?.opacity ?? null,
        vals?.translateX ?? null,
        vals?.translateY ?? null,
        vals?.scale ?? null,
        vals?.rotation ?? null,
        vals?.textColor ?? vals?.color ?? null,
      );
  }

  removeGlobals(): void {
    const names = [
      '__record_opacity', '__record_translate_x', '__record_translate_y',
      '__record_translate', '__record_scale', '__record_scale_x', '__record_scale_y',
      '__record_rotate', '__record_skew_x', '__record_skew_y', '__record_skew',
      '__record_position', '__record_left', '__record_top', '__record_right',
      '__record_bottom', '__record_width', '__record_height', '__record_padding',
      '__record_padding_x', '__record_padding_y', '__record_margin', '__record_margin_x',
      '__record_margin_y', '__record_flex_direction', '__record_justify_content',
      '__record_align_items', '__record_gap', '__record_flex_grow', '__record_bg',
      '__record_border_radius', '__record_border_width', '__record_border_top_width',
      '__record_border_right_width', '__record_border_bottom_width',
      '__record_border_left_width', '__record_border_style', '__record_border_color',
      '__record_stroke_width', '__record_stroke_color', '__record_fill_color',
      '__record_object_fit', '__record_text_color', '__record_text_size',
      '__record_font_weight', '__record_letter_spacing', '__record_text_align',
      '__record_line_height', '__record_shadow', '__record_shadow_color',
      '__record_inset_shadow', '__record_inset_shadow_color', '__record_drop_shadow',
      '__record_drop_shadow_color', '__record_text_content', '__record_stroke_dasharray',
      '__record_stroke_dashoffset', '__record_svg_path', '__record_text_unit_override',
      '__text_source_get', '__text_source_set', '__text_units_describe',
      '__canvas_save', '__canvas_restore', '__canvas_translate', '__canvas_scale',
      '__canvas_rotate', '__canvas_clip_rect', '__canvas_fill_rect',
      '__canvas_fill_rrect', '__canvas_fill_circle', '__canvas_stroke_circle',
      '__canvas_draw_line', '__canvas_begin_path', '__canvas_move_to',
      '__canvas_line_to', '__canvas_quad_to', '__canvas_cubic_to',
      '__canvas_close_path', '__canvas_fill_path', '__canvas_stroke_path',
      '__canvas_set_fill_style', '__canvas_set_stroke_style', '__canvas_set_line_width',
      '__canvas_set_global_alpha', '__canvas_clear', '__canvas_draw_image',
      '__canvas_draw_image_simple', '__canvas_draw_text',
      '__animate_create', '__animate_value', '__animate_color', '__animate_progress',
      '__animate_settled', '__animate_settle_frame', '__animate_dispose',
      '__flush_timelines', '__morph_svg_create', '__morph_svg_sample',
      '__morph_svg_dispose', '__along_path_create', '__along_path_length',
      '__along_path_at', '__along_path_dispose', '__util_random_seeded', '__easing_apply',
    ];
    for (const n of names) delete (window as any)[n];
  }
}
