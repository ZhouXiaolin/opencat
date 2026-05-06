/// <reference types="vite/client" />

declare module '../wasm/opencat_web.js' {
  export default function init(): Promise<void>;
  export function parse_jsonl(input: string): string;
  export function get_composition_info(input: string): string;
  export function collect_resources_json(input: string): string;
  export function build_frame(jsonl_input: string, frame: number, resource_meta: string, mutations_json: string): string;

  export class WebMutationRecorder {
    constructor();
    reset_for_frame(frame: number): void;
    snapshot_mutations_json(): string;

    record_opacity(id: string, v: number): void;
    record_translate_x(id: string, v: number): void;
    record_translate_y(id: string, v: number): void;
    record_translate(id: string, x: number, y: number): void;
    record_scale(id: string, v: number): void;
    record_scale_x(id: string, v: number): void;
    record_scale_y(id: string, v: number): void;
    record_rotate(id: string, v: number): void;
    record_skew_x(id: string, v: number): void;
    record_skew_y(id: string, v: number): void;
    record_skew(id: string, x: number, y: number): void;
    record_position(id: string, v: string): void;
    record_left(id: string, v: number): void;
    record_top(id: string, v: number): void;
    record_right(id: string, v: number): void;
    record_bottom(id: string, v: number): void;
    record_width(id: string, v: number): void;
    record_height(id: string, v: number): void;
    record_padding(id: string, v: number): void;
    record_padding_x(id: string, v: number): void;
    record_padding_y(id: string, v: number): void;
    record_margin(id: string, v: number): void;
    record_margin_x(id: string, v: number): void;
    record_margin_y(id: string, v: number): void;
    record_flex_direction(id: string, v: string): void;
    record_justify_content(id: string, v: string): void;
    record_align_items(id: string, v: string): void;
    record_gap(id: string, v: number): void;
    record_flex_grow(id: string, v: number): void;
    record_bg_color(id: string, v: string): void;
    record_border_radius(id: string, v: number): void;
    record_border_width(id: string, v: number): void;
    record_border_top_width(id: string, v: number): void;
    record_border_right_width(id: string, v: number): void;
    record_border_bottom_width(id: string, v: number): void;
    record_border_left_width(id: string, v: number): void;
    record_border_style(id: string, v: string): void;
    record_border_color(id: string, v: string): void;
    record_stroke_width(id: string, v: number): void;
    record_stroke_color(id: string, v: string): void;
    record_fill_color(id: string, v: string): void;
    record_object_fit(id: string, v: string): void;
    record_text_color(id: string, v: string): void;
    record_text_size(id: string, v: number): void;
    record_font_weight(id: string, v: number): void;
    record_letter_spacing(id: string, v: number): void;
    record_text_align(id: string, v: string): void;
    record_line_height(id: string, v: number): void;
    record_box_shadow(id: string, v: string): void;
    record_box_shadow_color(id: string, v: string): void;
    record_inset_shadow(id: string, v: string): void;
    record_inset_shadow_color(id: string, v: string): void;
    record_drop_shadow(id: string, v: string): void;
    record_drop_shadow_color(id: string, v: string): void;
    record_text_content(id: string, v: string): void;
    record_stroke_dasharray(id: string, v: number): void;
    record_stroke_dashoffset(id: string, v: number): void;
    record_svg_path(id: string, v: string): void;

    get_text(id: string): string;
    set_text(id: string, t: string): void;

    record_canvas_save(id: string): void;
    record_canvas_restore(id: string): void;
    record_canvas_translate(id: string, x: number, y: number): void;
    record_canvas_scale(id: string, x: number, y: number): void;
    record_canvas_rotate(id: string, deg: number): void;
    record_canvas_clip_rect(id: string, x: number, y: number, w: number, h: number, aa: boolean): void;
    record_canvas_fill_rect(id: string, x: number, y: number, w: number, h: number, color: string): void;
    record_canvas_fill_rrect(id: string, x: number, y: number, w: number, h: number, rad: number): void;
    record_canvas_fill_circle(id: string, cx: number, cy: number, rad: number): void;
    record_canvas_stroke_circle(id: string, cx: number, cy: number, rad: number): void;
    record_canvas_draw_line(id: string, x0: number, y0: number, x1: number, y1: number): void;
    record_canvas_begin_path(id: string): void;
    record_canvas_move_to(id: string, x: number, y: number): void;
    record_canvas_line_to(id: string, x: number, y: number): void;
    record_canvas_quad_to(id: string, cx: number, cy: number, x: number, y: number): void;
    record_canvas_cubic_to(id: string, c1x: number, c1y: number, c2x: number, c2y: number, x: number, y: number): void;
    record_canvas_close_path(id: string): void;
    record_canvas_fill_path(id: string): void;
    record_canvas_stroke_path(id: string): void;
    record_canvas_set_fill_style(id: string, c: string): void;
    record_canvas_set_stroke_style(id: string, c: string): void;
    record_canvas_set_line_width(id: string, w: number): void;
    record_canvas_set_global_alpha(id: string, a: number): void;
    record_canvas_clear(id: string, c: string): void;
    record_canvas_draw_image(id: string, asset: string, x: number, y: number, w: number, h: number, alpha: number): void;
    record_canvas_draw_image_simple(id: string, asset: string, x: number, y: number, alpha: number): void;
    record_canvas_draw_text(id: string, txt: string, x: number, y: number, fs: number, aa: boolean, stroke: boolean, sw: number, color: string): void;

    animate_create(frame: number, dur: number, del: number, clamp: boolean, ease: string, rep: number, yoyo: boolean, rd: number): number;
    animate_value(frame: number, h: number, from: number, to: number): number;
    animate_color(h: number, from: string, to: string): string;
    animate_progress(h: number): number;
    animate_settled(h: number): boolean;
    animate_settle_frame(h: number): number;

    morph_svg_create(from: string, to: string, grid: number): number;
    morph_svg_sample(h: number, t: number, tol: number): string;
    morph_svg_dispose(h: number): void;

    along_path_create(svg: string): number;
    along_path_length(h: number): number;
    along_path_at(h: number, t: number): Float64Array;
    along_path_dispose(h: number): void;

    random_from_seed(seed: number): number;
    easing_apply(tag: string, t: number): number;

    text_units_describe(id: string, gran: string): string;
    record_text_unit_override(id: string, gran: string, idx: number, opacity: number | null, tx: number | null, ty: number | null, scale: number | null, rotation: number | null, color: string | null): void;
  }
}
