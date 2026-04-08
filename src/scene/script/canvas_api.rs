use rquickjs::Function;

use super::{MutationStore, object_fit_from_name};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScriptColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScriptLineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScriptLineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CanvasCommand {
    Save,
    Restore,
    SetFillStyle {
        color: ScriptColor,
    },
    SetStrokeStyle {
        color: ScriptColor,
    },
    SetLineWidth {
        width: f32,
    },
    SetLineCap {
        cap: ScriptLineCap,
    },
    SetLineJoin {
        join: ScriptLineJoin,
    },
    SetLineDash {
        intervals: Vec<f32>,
        phase: f32,
    },
    ClearLineDash,
    SetGlobalAlpha {
        alpha: f32,
    },
    Translate {
        x: f32,
        y: f32,
    },
    Scale {
        x: f32,
        y: f32,
    },
    Rotate {
        degrees: f32,
    },
    ClipRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    Clear {
        color: Option<ScriptColor>,
    },
    FillRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: ScriptColor,
    },
    FillRRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
    },
    StrokeRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: ScriptColor,
        stroke_width: f32,
    },
    StrokeRRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
    },
    DrawLine {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
    },
    FillCircle {
        cx: f32,
        cy: f32,
        radius: f32,
    },
    StrokeCircle {
        cx: f32,
        cy: f32,
        radius: f32,
    },
    BeginPath,
    MoveTo {
        x: f32,
        y: f32,
    },
    LineTo {
        x: f32,
        y: f32,
    },
    QuadTo {
        cx: f32,
        cy: f32,
        x: f32,
        y: f32,
    },
    CubicTo {
        c1x: f32,
        c1y: f32,
        c2x: f32,
        c2y: f32,
        x: f32,
        y: f32,
    },
    ClosePath,
    FillPath,
    StrokePath,
    DrawImage {
        asset_id: String,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        object_fit: crate::style::ObjectFit,
    },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CanvasMutations {
    pub commands: Vec<CanvasCommand>,
}

fn line_cap_from_name(name: &str) -> Option<ScriptLineCap> {
    match name {
        "butt" => Some(ScriptLineCap::Butt),
        "round" => Some(ScriptLineCap::Round),
        "square" => Some(ScriptLineCap::Square),
        _ => None,
    }
}

fn line_join_from_name(name: &str) -> Option<ScriptLineJoin> {
    match name {
        "miter" => Some(ScriptLineJoin::Miter),
        "round" => Some(ScriptLineJoin::Round),
        "bevel" => Some(ScriptLineJoin::Bevel),
        _ => None,
    }
}

fn parse_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_rgb_channel(value: &str) -> Option<u8> {
    let channel = value.trim().parse::<f32>().ok()?;
    if !(0.0..=255.0).contains(&channel) {
        return None;
    }
    Some(channel.round() as u8)
}

fn parse_alpha_channel(value: &str) -> Option<u8> {
    let alpha = value.trim().parse::<f32>().ok()?;
    if !(0.0..=1.0).contains(&alpha) {
        return None;
    }
    Some((alpha * 255.0).round() as u8)
}

fn parse_rgb_function(value: &str) -> Option<ScriptColor> {
    let (is_rgba, body) = if let Some(body) = value
        .strip_prefix("rgba(")
        .and_then(|body| body.strip_suffix(')'))
    {
        (true, body)
    } else if let Some(body) = value
        .strip_prefix("rgb(")
        .and_then(|body| body.strip_suffix(')'))
    {
        (false, body)
    } else {
        return None;
    };

    let parts: Vec<_> = body.split(',').map(str::trim).collect();
    if (!is_rgba && parts.len() != 3) || (is_rgba && parts.len() != 4) {
        return None;
    }

    let r = parse_rgb_channel(parts[0])?;
    let g = parse_rgb_channel(parts[1])?;
    let b = parse_rgb_channel(parts[2])?;
    let a = if is_rgba {
        parse_alpha_channel(parts[3])?
    } else {
        255
    };

    Some(ScriptColor { r, g, b, a })
}

fn script_color_from_value(value: &str) -> Option<ScriptColor> {
    let color = crate::style::color_token_from_script_name(value).map(|color| color.rgba());
    if let Some((r, g, b, a)) = color {
        return Some(ScriptColor { r, g, b, a });
    }

    if let Some(color) = parse_rgb_function(value) {
        return Some(color);
    }

    let hex = value.strip_prefix('#')?;
    let (r, g, b, a) = match hex.len() {
        3 => {
            let r = parse_hex_nibble(hex.as_bytes()[0])?;
            let g = parse_hex_nibble(hex.as_bytes()[1])?;
            let b = parse_hex_nibble(hex.as_bytes()[2])?;
            (r * 17, g * 17, b * 17, 255)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b, 255)
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            (r, g, b, a)
        }
        _ => return None,
    };

    Some(ScriptColor { r, g, b, a })
}

pub(super) const CANVASKIT_RUNTIME: &str = r#"
(function() {
    const canvasCache = {};

    function clamp(value, min, max) {
        return Math.min(max, Math.max(min, Number(value)));
    }

    function toFiniteNumber(value, fallback = 0) {
        const number = Number(value);
        return Number.isFinite(number) ? number : fallback;
    }

    function isArrayLike(value) {
        return Array.isArray(value) || ArrayBuffer.isView(value);
    }

    function cloneColor(color) {
        return [color[0], color[1], color[2], color[3]];
    }

    function colorFromHex(hex) {
        const value = String(hex).trim().replace(/^#/, '');
        if (value.length === 3) {
            const r = parseInt(value[0] + value[0], 16);
            const g = parseInt(value[1] + value[1], 16);
            const b = parseInt(value[2] + value[2], 16);
            return [r / 255, g / 255, b / 255, 1];
        }
        if (value.length === 6) {
            const r = parseInt(value.slice(0, 2), 16);
            const g = parseInt(value.slice(2, 4), 16);
            const b = parseInt(value.slice(4, 6), 16);
            return [r / 255, g / 255, b / 255, 1];
        }
        if (value.length === 8) {
            const r = parseInt(value.slice(0, 2), 16);
            const g = parseInt(value.slice(2, 4), 16);
            const b = parseInt(value.slice(4, 6), 16);
            const a = parseInt(value.slice(6, 8), 16);
            return [r / 255, g / 255, b / 255, a / 255];
        }
        throw new Error(`unsupported color literal: ${hex}`);
    }

    function colorFromRgbFunction(input) {
        const value = String(input).trim();
        const rgba = value.match(/^rgba\((.+)\)$/i);
        const rgb = value.match(/^rgb\((.+)\)$/i);
        const body = rgba ? rgba[1] : rgb ? rgb[1] : null;
        if (!body) {
            return null;
        }
        const parts = body.split(',').map((part) => part.trim());
        if (parts.length !== (rgba ? 4 : 3)) {
            throw new Error(`unsupported color literal: ${input}`);
        }
        const r = clamp(parts[0], 0, 255) / 255;
        const g = clamp(parts[1], 0, 255) / 255;
        const b = clamp(parts[2], 0, 255) / 255;
        const a = rgba ? clamp(parts[3], 0, 1) : 1;
        return [r, g, b, a];
    }

    function parseColorString(input, colorMap) {
        const value = String(input).trim();
        if (colorMap && Object.prototype.hasOwnProperty.call(colorMap, value)) {
            return normalizeColor(colorMap[value]);
        }
        const lower = value.toLowerCase();
        if (lower === 'black') {
            return [0, 0, 0, 1];
        }
        if (lower === 'white') {
            return [1, 1, 1, 1];
        }
        if (lower.startsWith('#')) {
            return colorFromHex(lower);
        }
        const rgb = colorFromRgbFunction(lower);
        if (rgb) {
            return rgb;
        }
        throw new Error(`unsupported color literal: ${input}`);
    }

    function normalizeColor(value) {
        if (typeof value === 'string') {
            return parseColorString(value);
        }
        if (!isArrayLike(value) || value.length < 4) {
            throw new Error('expected an InputColor-compatible value');
        }
        return [
            clamp(value[0], 0, 1),
            clamp(value[1], 0, 1),
            clamp(value[2], 0, 1),
            clamp(value[3], 0, 1)
        ];
    }

    function colorToCss(value) {
        const color = normalizeColor(value);
        return `rgba(${Math.round(color[0] * 255)},${Math.round(color[1] * 255)},${Math.round(color[2] * 255)},${color[3]})`;
    }

    function normalizeRect(rect) {
        if (!isArrayLike(rect) || rect.length < 4) {
            throw new Error('expected an InputRect-compatible value');
        }
        const left = toFiniteNumber(rect[0]);
        const top = toFiniteNumber(rect[1]);
        const right = toFiniteNumber(rect[2]);
        const bottom = toFiniteNumber(rect[3]);
        return {
            left,
            top,
            right,
            bottom,
            x: left,
            y: top,
            width: right - left,
            height: bottom - top
        };
    }

    function normalizeRRect(rrect) {
        if (rrect && rrect.__opencatRRect === true) {
            const rect = normalizeRect(rrect.rect);
            return {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
                radius: Math.min(
                    Math.abs(toFiniteNumber(rrect.rx)),
                    Math.abs(toFiniteNumber(rrect.ry))
                )
            };
        }
        if (isArrayLike(rrect) && rrect.length >= 12) {
            const rect = normalizeRect(rrect);
            return {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
                radius: Math.min(
                    Math.abs(toFiniteNumber(rrect[4])),
                    Math.abs(toFiniteNumber(rrect[5]))
                )
            };
        }
        throw new Error('expected an InputRRect-compatible value');
    }

    function normalizePaintStyle(value) {
        if (value === CanvasKit.PaintStyle.Fill || value === 'fill') {
            return CanvasKit.PaintStyle.Fill;
        }
        if (value === CanvasKit.PaintStyle.Stroke || value === 'stroke') {
            return CanvasKit.PaintStyle.Stroke;
        }
        throw new Error(`unsupported PaintStyle: ${value}`);
    }

    function normalizeStrokeCap(value) {
        if (value === CanvasKit.StrokeCap.Butt || value === 'butt') {
            return CanvasKit.StrokeCap.Butt;
        }
        if (value === CanvasKit.StrokeCap.Round || value === 'round') {
            return CanvasKit.StrokeCap.Round;
        }
        if (value === CanvasKit.StrokeCap.Square || value === 'square') {
            return CanvasKit.StrokeCap.Square;
        }
        throw new Error(`unsupported StrokeCap: ${value}`);
    }

    function normalizeStrokeJoin(value) {
        if (value === CanvasKit.StrokeJoin.Miter || value === 'miter') {
            return CanvasKit.StrokeJoin.Miter;
        }
        if (value === CanvasKit.StrokeJoin.Round || value === 'round') {
            return CanvasKit.StrokeJoin.Round;
        }
        if (value === CanvasKit.StrokeJoin.Bevel || value === 'bevel') {
            return CanvasKit.StrokeJoin.Bevel;
        }
        throw new Error(`unsupported StrokeJoin: ${value}`);
    }

    function ensurePaint(paint) {
        if (!(paint instanceof Paint)) {
            throw new Error('expected a CanvasKit.Paint instance');
        }
        return paint;
    }

    function ensurePath(path) {
        if (!(path instanceof Path)) {
            throw new Error('expected a CanvasKit.Path instance');
        }
        return path;
    }

    function ensureImage(image) {
        if (!image || image.__opencatImage !== true) {
            throw new Error('expected an image from ctx.getImage(assetId)');
        }
        return image;
    }

    class Paint {
        constructor() {
            this._color = [0, 0, 0, 1];
            this._style = 'fill';
            this._strokeWidth = 1;
            this._strokeCap = 'butt';
            this._strokeJoin = 'miter';
            this._strokeDash = null;
            this._antiAlias = true;
        }

        copy() {
            const copy = new Paint();
            copy._color = cloneColor(this._color);
            copy._style = this._style;
            copy._strokeWidth = this._strokeWidth;
            copy._strokeCap = this._strokeCap;
            copy._strokeJoin = this._strokeJoin;
            copy._strokeDash = this._strokeDash
                ? {
                    intervals: this._strokeDash.intervals.slice(),
                    phase: this._strokeDash.phase
                }
                : null;
            copy._antiAlias = this._antiAlias;
            return copy;
        }

        delete() {}

        getColor() {
            return cloneColor(this._color);
        }

        getStrokeCap() {
            return this._strokeCap;
        }

        getStrokeJoin() {
            return this._strokeJoin;
        }

        getStrokeWidth() {
            return this._strokeWidth;
        }

        setAlphaf(alpha) {
            this._color[3] = clamp(alpha, 0, 1);
        }

        setAntiAlias(aa) {
            this._antiAlias = !!aa;
        }

        setColor(color) {
            this._color = normalizeColor(color);
        }

        setColorComponents(r, g, b, a = 1) {
            this._color = [
                clamp(r, 0, 1),
                clamp(g, 0, 1),
                clamp(b, 0, 1),
                clamp(a, 0, 1)
            ];
        }

        setColorInt(color) {
            const value = Number(color) >>> 0;
            this._color = [
                ((value >>> 16) & 0xff) / 255,
                ((value >>> 8) & 0xff) / 255,
                (value & 0xff) / 255,
                ((value >>> 24) & 0xff) / 255
            ];
        }

        setStrokeCap(cap) {
            this._strokeCap = normalizeStrokeCap(cap);
        }

        setStrokeDash(intervals, phase = 0) {
            if (!Array.isArray(intervals) || intervals.length < 2) {
                throw new Error('setStrokeDash expects at least two dash intervals');
            }
            const normalized = intervals.map((value) => {
                const n = toFiniteNumber(value);
                if (n <= 0) {
                    throw new Error('setStrokeDash intervals must be positive');
                }
                return n;
            });
            this._strokeDash = {
                intervals: normalized,
                phase: toFiniteNumber(phase, 0)
            };
            return this;
        }

        setStrokeJoin(join) {
            this._strokeJoin = normalizeStrokeJoin(join);
        }

        setStrokeWidth(width) {
            this._strokeWidth = Math.max(0, toFiniteNumber(width, 1));
        }

        setStyle(style) {
            this._style = normalizePaintStyle(style);
        }
    }

    class Path {
        constructor() {
            this._ops = [];
        }

        copy() {
            const copy = new Path();
            copy._ops = this._ops.map((op) => op.slice());
            return copy;
        }

        delete() {}

        moveTo(x, y) {
            this._ops.push(['moveTo', toFiniteNumber(x), toFiniteNumber(y)]);
            return this;
        }

        lineTo(x, y) {
            this._ops.push(['lineTo', toFiniteNumber(x), toFiniteNumber(y)]);
            return this;
        }

        quadTo(x1, y1, x2, y2) {
            this._ops.push([
                'quadTo',
                toFiniteNumber(x1),
                toFiniteNumber(y1),
                toFiniteNumber(x2),
                toFiniteNumber(y2)
            ]);
            return this;
        }

        cubicTo(x1, y1, x2, y2, x3, y3) {
            this._ops.push([
                'cubicTo',
                toFiniteNumber(x1),
                toFiniteNumber(y1),
                toFiniteNumber(x2),
                toFiniteNumber(y2),
                toFiniteNumber(x3),
                toFiniteNumber(y3)
            ]);
            return this;
        }

        close() {
            this._ops.push(['close']);
            return this;
        }
    }

    const CanvasKit = {
        Color(r, g, b, a = 1) {
            return [
                clamp(r, 0, 255) / 255,
                clamp(g, 0, 255) / 255,
                clamp(b, 0, 255) / 255,
                clamp(a, 0, 1)
            ];
        },
        Color4f(r, g, b, a = 1) {
            return [
                clamp(r, 0, 1),
                clamp(g, 0, 1),
                clamp(b, 0, 1),
                clamp(a, 0, 1)
            ];
        },
        ColorAsInt(r, g, b, a = 1) {
            return (
                ((Math.round(clamp(a, 0, 1) * 255) & 0xff) << 24) |
                ((Math.round(clamp(r, 0, 255)) & 0xff) << 16) |
                ((Math.round(clamp(g, 0, 255)) & 0xff) << 8) |
                (Math.round(clamp(b, 0, 255)) & 0xff)
            ) >>> 0;
        },
        parseColorString,
        multiplyByAlpha(color, alpha) {
            const normalized = normalizeColor(color);
            return [
                normalized[0],
                normalized[1],
                normalized[2],
                clamp(normalized[3] * toFiniteNumber(alpha, 1), 0, 1)
            ];
        },
        LTRBRect(left, top, right, bottom) {
            return [
                toFiniteNumber(left),
                toFiniteNumber(top),
                toFiniteNumber(right),
                toFiniteNumber(bottom)
            ];
        },
        XYWHRect(x, y, width, height) {
            const left = toFiniteNumber(x);
            const top = toFiniteNumber(y);
            return [
                left,
                top,
                left + toFiniteNumber(width),
                top + toFiniteNumber(height)
            ];
        },
        RRectXY(rect, rx, ry) {
            const normalized = normalizeRect(rect);
            return {
                __opencatRRect: true,
                rect: [normalized.left, normalized.top, normalized.right, normalized.bottom],
                rx: toFiniteNumber(rx),
                ry: toFiniteNumber(ry)
            };
        },
        Paint,
        Path,
        PaintStyle: {
            Fill: 'fill',
            Stroke: 'stroke'
        },
        StrokeCap: {
            Butt: 'butt',
            Round: 'round',
            Square: 'square'
        },
        StrokeJoin: {
            Miter: 'miter',
            Round: 'round',
            Bevel: 'bevel'
        },
        ClipOp: {
            Difference: 'difference',
            Intersect: 'intersect'
        },
        BLACK: [0, 0, 0, 1],
        WHITE: [1, 1, 1, 1]
    };

    function applyFillPaint(id, paint) {
        __canvas_set_fill_style(id, colorToCss(paint._color));
    }

    function applyStrokePaint(id, paint) {
        __canvas_set_stroke_style(id, colorToCss(paint._color));
        __canvas_set_line_width(id, paint._strokeWidth);
        __canvas_set_line_cap(id, paint._strokeCap);
        __canvas_set_line_join(id, paint._strokeJoin);
        if (paint._strokeDash) {
            __canvas_set_line_dash(id, paint._strokeDash.intervals, paint._strokeDash.phase);
        } else {
            __canvas_clear_line_dash(id);
        }
    }

    function replayPath(id, path) {
        __canvas_begin_path(id);
        for (const op of path._ops) {
            switch (op[0]) {
                case 'moveTo':
                    __canvas_move_to(id, op[1], op[2]);
                    break;
                case 'lineTo':
                    __canvas_line_to(id, op[1], op[2]);
                    break;
                case 'quadTo':
                    __canvas_quad_to(id, op[1], op[2], op[3], op[4]);
                    break;
                case 'cubicTo':
                    __canvas_cubic_to(id, op[1], op[2], op[3], op[4], op[5], op[6]);
                    break;
                case 'close':
                    __canvas_close_path(id);
                    break;
                default:
                    throw new Error(`unsupported path verb: ${op[0]}`);
            }
        }
    }

    function makeCanvas(id) {
        return {
            clear(color) {
                if (arguments.length === 0 || color == null) {
                    __canvas_clear(id, null);
                } else {
                    __canvas_clear(id, colorToCss(color));
                }
                return this;
            },

            clipRect(rect, op = CanvasKit.ClipOp.Intersect) {
                if (op !== CanvasKit.ClipOp.Intersect) {
                    throw new Error('only CanvasKit.ClipOp.Intersect is supported');
                }
                const normalized = normalizeRect(rect);
                __canvas_clip_rect(
                    id,
                    normalized.x,
                    normalized.y,
                    normalized.width,
                    normalized.height
                );
                return this;
            },

            drawCircle(cx, cy, radius, paint) {
                const resolvedPaint = ensurePaint(paint);
                if (resolvedPaint._style === CanvasKit.PaintStyle.Stroke) {
                    applyStrokePaint(id, resolvedPaint);
                    __canvas_stroke_circle(
                        id,
                        toFiniteNumber(cx),
                        toFiniteNumber(cy),
                        Math.max(0, toFiniteNumber(radius))
                    );
                } else {
                    applyFillPaint(id, resolvedPaint);
                    __canvas_fill_circle(
                        id,
                        toFiniteNumber(cx),
                        toFiniteNumber(cy),
                        Math.max(0, toFiniteNumber(radius))
                    );
                }
                return this;
            },

            drawImageRect(image, _src, dest) {
                const resolvedImage = ensureImage(image);
                const normalized = normalizeRect(dest);
                __canvas_draw_image(
                    id,
                    resolvedImage.assetId,
                    normalized.x,
                    normalized.y,
                    normalized.width,
                    normalized.height,
                    'fill'
                );
                return this;
            },

            drawLine(x0, y0, x1, y1, paint) {
                const resolvedPaint = ensurePaint(paint);
                applyStrokePaint(id, resolvedPaint);
                __canvas_draw_line(
                    id,
                    toFiniteNumber(x0),
                    toFiniteNumber(y0),
                    toFiniteNumber(x1),
                    toFiniteNumber(y1)
                );
                return this;
            },

            drawPath(path, paint) {
                const resolvedPath = ensurePath(path);
                const resolvedPaint = ensurePaint(paint);
                if (resolvedPaint._style === CanvasKit.PaintStyle.Stroke) {
                    applyStrokePaint(id, resolvedPaint);
                    replayPath(id, resolvedPath);
                    __canvas_stroke_path(id);
                } else {
                    applyFillPaint(id, resolvedPaint);
                    replayPath(id, resolvedPath);
                    __canvas_fill_path(id);
                }
                return this;
            },

            drawRect(rect, paint) {
                const normalized = normalizeRect(rect);
                const resolvedPaint = ensurePaint(paint);
                if (resolvedPaint._style === CanvasKit.PaintStyle.Stroke) {
                    __canvas_stroke_rect(
                        id,
                        normalized.x,
                        normalized.y,
                        normalized.width,
                        normalized.height,
                        colorToCss(resolvedPaint._color),
                        resolvedPaint._strokeWidth
                    );
                } else {
                    __canvas_fill_rect(
                        id,
                        normalized.x,
                        normalized.y,
                        normalized.width,
                        normalized.height,
                        colorToCss(resolvedPaint._color)
                    );
                }
                return this;
            },

            drawRRect(rrect, paint) {
                const normalized = normalizeRRect(rrect);
                const resolvedPaint = ensurePaint(paint);
                if (resolvedPaint._style === CanvasKit.PaintStyle.Stroke) {
                    applyStrokePaint(id, resolvedPaint);
                    __canvas_stroke_rrect(
                        id,
                        normalized.x,
                        normalized.y,
                        normalized.width,
                        normalized.height,
                        normalized.radius
                    );
                } else {
                    applyFillPaint(id, resolvedPaint);
                    __canvas_fill_rrect(
                        id,
                        normalized.x,
                        normalized.y,
                        normalized.width,
                        normalized.height,
                        normalized.radius
                    );
                }
                return this;
            },

            restore() {
                __canvas_restore(id);
                return this;
            },

            rotate(degrees, rx, ry) {
                if (arguments.length >= 3) {
                    __canvas_translate(id, toFiniteNumber(rx), toFiniteNumber(ry));
                    __canvas_rotate(id, toFiniteNumber(degrees));
                    __canvas_translate(id, -toFiniteNumber(rx), -toFiniteNumber(ry));
                } else {
                    __canvas_rotate(id, toFiniteNumber(degrees));
                }
                return this;
            },

            save() {
                __canvas_save(id);
                return this;
            },

            setAlphaf(alpha) {
                __canvas_set_global_alpha(id, toFiniteNumber(alpha, 1));
                return this;
            },

            scale(sx, sy) {
                const scaleX = toFiniteNumber(sx, 1);
                const scaleY = arguments.length >= 2 ? toFiniteNumber(sy, 1) : scaleX;
                __canvas_scale(id, scaleX, scaleY);
                return this;
            },

            translate(dx, dy) {
                __canvas_translate(id, toFiniteNumber(dx), toFiniteNumber(dy));
                return this;
            }
        };
    }

    globalThis.CanvasKit = CanvasKit;
    ctx.CanvasKit = CanvasKit;
    ctx.getImage = function(assetId) {
        return {
            __opencatImage: true,
            assetId: String(assetId),
            delete() {}
        };
    };
    ctx.getCanvas = function() {
        const id = ctx.__currentCanvasTarget;
        if (!id) {
            return null;
        }
        if (!canvasCache[id]) {
            canvasCache[id] = makeCanvas(id);
        }
        return canvasCache[id];
    };
})();
"#;

pub(super) fn install_canvaskit_bindings<'js>(
    ctx: &rquickjs::Ctx<'js>,
    store: &MutationStore,
) -> anyhow::Result<()> {
    let globals = ctx.globals();

    macro_rules! push_canvas_command {
        ($name:literal, |$id:ident $(, $arg:ident : $arg_ty:ty)*| $command:expr) => {{
            let s = store.clone();
            globals.set(
                $name,
                Function::new(ctx.clone(), move |$id: String $(, $arg: $arg_ty)*| {
                    let mut map = s.lock().unwrap();
                    map.canvases
                        .entry($id)
                        .or_default()
                        .commands
                        .push($command);
                    Ok::<_, rquickjs::Error>(())
                })?,
            )?;
        }};
    }

    push_canvas_command!("__canvas_save", |id| CanvasCommand::Save);
    push_canvas_command!("__canvas_restore", |id| CanvasCommand::Restore);
    push_canvas_command!("__canvas_translate", |id, x: f32, y: f32| {
        CanvasCommand::Translate { x, y }
    });
    push_canvas_command!("__canvas_scale", |id, x: f32, y: f32| {
        CanvasCommand::Scale { x, y }
    });
    push_canvas_command!("__canvas_rotate", |id, degrees: f32| {
        CanvasCommand::Rotate { degrees }
    });
    push_canvas_command!(
        "__canvas_clip_rect",
        |id, x: f32, y: f32, width: f32, height: f32| CanvasCommand::ClipRect {
            x,
            y,
            width,
            height
        }
    );
    push_canvas_command!(
        "__canvas_draw_line",
        |id, x0: f32, y0: f32, x1: f32, y1: f32| CanvasCommand::DrawLine { x0, y0, x1, y1 }
    );
    push_canvas_command!(
        "__canvas_fill_circle",
        |id, cx: f32, cy: f32, radius: f32| CanvasCommand::FillCircle { cx, cy, radius }
    );
    push_canvas_command!(
        "__canvas_stroke_circle",
        |id, cx: f32, cy: f32, radius: f32| CanvasCommand::StrokeCircle { cx, cy, radius }
    );
    push_canvas_command!(
        "__canvas_fill_rrect",
        |id, x: f32, y: f32, width: f32, height: f32, radius: f32| CanvasCommand::FillRRect {
            x,
            y,
            width,
            height,
            radius
        }
    );
    push_canvas_command!(
        "__canvas_stroke_rrect",
        |id, x: f32, y: f32, width: f32, height: f32, radius: f32| CanvasCommand::StrokeRRect {
            x,
            y,
            width,
            height,
            radius
        }
    );
    push_canvas_command!("__canvas_begin_path", |id| CanvasCommand::BeginPath);
    push_canvas_command!("__canvas_move_to", |id, x: f32, y: f32| {
        CanvasCommand::MoveTo { x, y }
    });
    push_canvas_command!("__canvas_line_to", |id, x: f32, y: f32| {
        CanvasCommand::LineTo { x, y }
    });
    push_canvas_command!(
        "__canvas_quad_to",
        |id, cx: f32, cy: f32, x: f32, y: f32| CanvasCommand::QuadTo { cx, cy, x, y }
    );
    push_canvas_command!(
        "__canvas_cubic_to",
        |id, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32| CanvasCommand::CubicTo {
            c1x,
            c1y,
            c2x,
            c2y,
            x,
            y
        }
    );
    push_canvas_command!("__canvas_close_path", |id| CanvasCommand::ClosePath);
    push_canvas_command!("__canvas_fill_path", |id| CanvasCommand::FillPath);
    push_canvas_command!("__canvas_stroke_path", |id| CanvasCommand::StrokePath);

    let s = store.clone();
    globals.set(
        "__canvas_set_fill_style",
        Function::new(ctx.clone(), move |id: String, color: String| {
            let color = parse_color(&color, "setFillStyle")?;
            let mut map = s.lock().unwrap();
            map.canvases
                .entry(id)
                .or_default()
                .commands
                .push(CanvasCommand::SetFillStyle { color });
            Ok::<_, rquickjs::Error>(())
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_set_stroke_style",
        Function::new(ctx.clone(), move |id: String, color: String| {
            let color = parse_color(&color, "setStrokeStyle")?;
            let mut map = s.lock().unwrap();
            map.canvases
                .entry(id)
                .or_default()
                .commands
                .push(CanvasCommand::SetStrokeStyle { color });
            Ok::<_, rquickjs::Error>(())
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_set_line_width",
        Function::new(ctx.clone(), move |id: String, width: f32| {
            let mut map = s.lock().unwrap();
            map.canvases
                .entry(id)
                .or_default()
                .commands
                .push(CanvasCommand::SetLineWidth {
                    width: width.max(0.0),
                });
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_set_line_cap",
        Function::new(ctx.clone(), move |id: String, cap: String| {
            let cap = line_cap_from_name(&cap)
                .ok_or_else(|| js_error("setLineCap", format!("unsupported line cap `{cap}`")))?;
            let mut map = s.lock().unwrap();
            map.canvases
                .entry(id)
                .or_default()
                .commands
                .push(CanvasCommand::SetLineCap { cap });
            Ok::<_, rquickjs::Error>(())
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_set_line_join",
        Function::new(ctx.clone(), move |id: String, join: String| {
            let join = line_join_from_name(&join).ok_or_else(|| {
                js_error("setLineJoin", format!("unsupported line join `{join}`"))
            })?;
            let mut map = s.lock().unwrap();
            map.canvases
                .entry(id)
                .or_default()
                .commands
                .push(CanvasCommand::SetLineJoin { join });
            Ok::<_, rquickjs::Error>(())
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_set_line_dash",
        Function::new(
            ctx.clone(),
            move |id: String, intervals: Vec<f32>, phase: f32| {
                let mut map = s.lock().unwrap();
                map.canvases
                    .entry(id)
                    .or_default()
                    .commands
                    .push(CanvasCommand::SetLineDash { intervals, phase });
            },
        )?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_clear_line_dash",
        Function::new(ctx.clone(), move |id: String| {
            let mut map = s.lock().unwrap();
            map.canvases
                .entry(id)
                .or_default()
                .commands
                .push(CanvasCommand::ClearLineDash);
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_set_global_alpha",
        Function::new(ctx.clone(), move |id: String, alpha: f32| {
            let mut map = s.lock().unwrap();
            map.canvases
                .entry(id)
                .or_default()
                .commands
                .push(CanvasCommand::SetGlobalAlpha {
                    alpha: alpha.clamp(0.0, 1.0),
                });
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_clear",
        Function::new(ctx.clone(), move |id: String, color: Option<String>| {
            let color = match color {
                Some(color) => Some(parse_color(&color, "clear")?),
                None => None,
            };
            let mut map = s.lock().unwrap();
            map.canvases
                .entry(id)
                .or_default()
                .commands
                .push(CanvasCommand::Clear { color });
            Ok::<_, rquickjs::Error>(())
        })?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_fill_rect",
        Function::new(
            ctx.clone(),
            move |id: String, x: f32, y: f32, width: f32, height: f32, color: String| {
                let color = parse_color(&color, "fillRect")?;
                let mut map = s.lock().unwrap();
                map.canvases
                    .entry(id)
                    .or_default()
                    .commands
                    .push(CanvasCommand::FillRect {
                        x,
                        y,
                        width,
                        height,
                        color,
                    });
                Ok::<_, rquickjs::Error>(())
            },
        )?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_stroke_rect",
        Function::new(
            ctx.clone(),
            move |id: String,
                  x: f32,
                  y: f32,
                  width: f32,
                  height: f32,
                  color: String,
                  stroke_width: f32| {
                let color = parse_color(&color, "strokeRect")?;
                let mut map = s.lock().unwrap();
                map.canvases
                    .entry(id)
                    .or_default()
                    .commands
                    .push(CanvasCommand::StrokeRect {
                        x,
                        y,
                        width,
                        height,
                        color,
                        stroke_width: stroke_width.max(0.0),
                    });
                Ok::<_, rquickjs::Error>(())
            },
        )?,
    )?;

    let s = store.clone();
    globals.set(
        "__canvas_draw_image",
        Function::new(
            ctx.clone(),
            move |id: String,
                  asset_id: String,
                  x: f32,
                  y: f32,
                  width: f32,
                  height: f32,
                  fit: String| {
                let object_fit = object_fit_from_name(&fit).ok_or_else(|| {
                    js_error("drawImage", format!("unsupported objectFit `{fit}`"))
                })?;
                let mut map = s.lock().unwrap();
                map.canvases
                    .entry(id)
                    .or_default()
                    .commands
                    .push(CanvasCommand::DrawImage {
                        asset_id,
                        x,
                        y,
                        width,
                        height,
                        object_fit,
                    });
                Ok::<_, rquickjs::Error>(())
            },
        )?,
    )?;

    Ok(())
}

fn js_error(op: &'static str, message: String) -> rquickjs::Error {
    rquickjs::Error::new_from_js_message("canvas", op, message)
}

fn parse_color(color: &str, op: &'static str) -> Result<ScriptColor, rquickjs::Error> {
    script_color_from_value(color)
        .ok_or_else(|| js_error(op, format!("unsupported color `{color}`")))
}
