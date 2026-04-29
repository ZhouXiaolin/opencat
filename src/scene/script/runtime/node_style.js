(function() {
    function applyMutation(id, prop, ...args) {
        switch (prop) {
            case 'opacity': __record_opacity(id, args[0]); break;
            case 'translateX': __record_translate_x(id, args[0]); break;
            case 'translateY': __record_translate_y(id, args[0]); break;
            case 'translate': __record_translate(id, args[0], args[1]); break;
            case 'scale': __record_scale(id, args[0]); break;
            case 'scaleX': __record_scale_x(id, args[0]); break;
            case 'scaleY': __record_scale_y(id, args[0]); break;
            case 'rotate': __record_rotate(id, args[0]); break;
            case 'skewX': __record_skew_x(id, args[0]); break;
            case 'skewY': __record_skew_y(id, args[0]); break;
            case 'skew': __record_skew(id, args[0], args[1]); break;
            case 'position': __record_position(id, String(args[0])); break;
            case 'left': __record_left(id, args[0]); break;
            case 'top': __record_top(id, args[0]); break;
            case 'right': __record_right(id, args[0]); break;
            case 'bottom': __record_bottom(id, args[0]); break;
            case 'width': __record_width(id, args[0]); break;
            case 'height': __record_height(id, args[0]); break;
            case 'padding': __record_padding(id, args[0]); break;
            case 'paddingX': __record_padding_x(id, args[0]); break;
            case 'paddingY': __record_padding_y(id, args[0]); break;
            case 'margin': __record_margin(id, args[0]); break;
            case 'marginX': __record_margin_x(id, args[0]); break;
            case 'marginY': __record_margin_y(id, args[0]); break;
            case 'flexDirection': __record_flex_direction(id, String(args[0])); break;
            case 'justifyContent': __record_justify_content(id, String(args[0])); break;
            case 'alignItems': __record_align_items(id, String(args[0])); break;
            case 'gap': __record_gap(id, args[0]); break;
            case 'flexGrow': __record_flex_grow(id, args[0]); break;
            case 'bg': __record_bg(id, String(args[0])); break;
            case 'borderRadius': __record_border_radius(id, args[0]); break;
            case 'borderWidth': __record_border_width(id, args[0]); break;
            case 'borderTopWidth': __record_border_top_width(id, args[0]); break;
            case 'borderRightWidth': __record_border_right_width(id, args[0]); break;
            case 'borderBottomWidth': __record_border_bottom_width(id, args[0]); break;
            case 'borderLeftWidth': __record_border_left_width(id, args[0]); break;
            case 'borderStyle': __record_border_style(id, String(args[0])); break;
            case 'borderColor': __record_border_color(id, String(args[0])); break;
            case 'strokeWidth': __record_stroke_width(id, args[0]); break;
            case 'strokeColor': __record_stroke_color(id, String(args[0])); break;
            case 'fillColor': __record_fill_color(id, String(args[0])); break;
            case 'objectFit': __record_object_fit(id, String(args[0])); break;
            case 'textColor': __record_text_color(id, String(args[0])); break;
            case 'textSize': __record_text_size(id, args[0]); break;
            case 'fontWeight': __record_font_weight(id, String(args[0])); break;
            case 'letterSpacing': __record_letter_spacing(id, args[0]); break;
            case 'textAlign': __record_text_align(id, String(args[0])); break;
            case 'lineHeight': __record_line_height(id, args[0]); break;
            case 'shadow': __record_shadow(id, String(args[0])); break;
            case 'shadowColor': __record_shadow_color(id, String(args[0])); break;
            case 'insetShadow': __record_inset_shadow(id, String(args[0])); break;
            case 'insetShadowColor': __record_inset_shadow_color(id, String(args[0])); break;
            case 'dropShadow': __record_drop_shadow(id, String(args[0])); break;
            case 'dropShadowColor': __record_drop_shadow_color(id, String(args[0])); break;
            case 'text': __record_text_content(id, String(args[0])); break;
            case 'svgPath': __record_svg_path(id, String(args[0])); break;
        }
    }

    const nodeCache = {};
    ctx.getNode = function(id) {
        if (!nodeCache[id]) {
            let api = null;
            api = new Proxy({}, {
                get(target, prop) {
                    if (typeof prop !== 'string' || prop === 'then') {
                        return undefined;
                    }
                    return (...args) => {
                        applyMutation(id, prop, ...args);
                        return api;
                    };
                }
            });
            nodeCache[id] = api;
        }
        return nodeCache[id];
    };
})();
