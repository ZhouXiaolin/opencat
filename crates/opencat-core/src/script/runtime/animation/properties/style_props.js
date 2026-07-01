(function() {
    var animation = globalThis.__opencatAnimation.animation;
    var copyOwn = globalThis.__opencatAnimation.copyOwn;

    function makeStyleProperty(nodeSetter, objectKey, defaultValue) {
        return {
            defaultValue: defaultValue,
            interpolate: 'number',
            apply: function(target, value) {
                if (target.node) {
                    target.node[nodeSetter](value);
                    return;
                }
                if (target.set) {
                    var values = {};
                    values[objectKey] = value;
                    target.set(values);
                    return;
                }
                throw new Error('target does not accept style property `' + objectKey + '`');
            },
        };
    }

    function parseInsetClipPath(value) {
        if (value == null || value === '' || value === 'none') {
            return null;
        }
        var match = String(value).trim().match(/^inset\((.*)\)$/i);
        if (!match) {
            return null;
        }
        var body = match[1].split(/\s+round\s+/i)[0].trim();
        if (!body) {
            return null;
        }
        var parts = body.split(/\s+/).map(parseLengthPercentage);
        for (var i = 0; i < parts.length; i++) {
            if (!parts[i]) return null;
        }
        if (parts.length === 1) {
            return [parts[0], parts[0], parts[0], parts[0]];
        }
        if (parts.length === 2) {
            return [parts[0], parts[1], parts[0], parts[1]];
        }
        if (parts.length === 3) {
            return [parts[0], parts[1], parts[2], parts[1]];
        }
        if (parts.length === 4) {
            return parts;
        }
        return null;
    }

    function parseLengthPercentage(raw) {
        raw = String(raw).trim();
        if (!raw) return null;
        if (raw.charAt(raw.length - 1) === '%') {
            var pct = Number(raw.slice(0, -1));
            return isNaN(pct) ? null : { value: pct, unit: '%' };
        }
        if (raw.slice(-2) === 'px') {
            raw = raw.slice(0, -2);
        }
        var px = Number(raw);
        return isNaN(px) ? null : { value: px, unit: 'px' };
    }

    function formatLengthPercentage(part) {
        var value = Number(part.value);
        var rounded = Math.abs(value - Math.round(value)) < 1e-6 ? String(Math.round(value)) : String(value);
        return rounded + part.unit;
    }

    function interpolateClipPath(from, to, progress) {
        var a = parseInsetClipPath(from) || parseInsetClipPath('inset(0% 0% 0% 0%)');
        var b = parseInsetClipPath(to);
        if (!b) {
            return progress < 1 ? String(from || 'inset(0% 0% 0% 0%)') : String(to || 'none');
        }
        var out = [];
        for (var i = 0; i < 4; i++) {
            if (a[i].unit !== b[i].unit) {
                out.push(progress < 0.5 ? a[i] : b[i]);
            } else {
                out.push({
                    value: a[i].value + (b[i].value - a[i].value) * progress,
                    unit: a[i].unit,
                });
            }
        }
        return 'inset(' + out.map(formatLengthPercentage).join(' ') + ')';
    }

    // Transform
    animation.registerProperty('opacity', makeStyleProperty('opacity', 'opacity', 1));
    animation.registerProperty('x', copyOwn(makeStyleProperty('translateX', 'x', 0), { aliases: ['translateX'] }));
    animation.registerProperty('y', copyOwn(makeStyleProperty('translateY', 'y', 0), { aliases: ['translateY'] }));
    animation.registerProperty('scale', makeStyleProperty('scale', 'scale', 1));
    animation.registerProperty('scaleX', makeStyleProperty('scaleX', 'scaleX', 1));
    animation.registerProperty('scaleY', makeStyleProperty('scaleY', 'scaleY', 1));
    animation.registerProperty('rotation', copyOwn(makeStyleProperty('rotate', 'rotation', 0), { aliases: ['rotate'] }));
    animation.registerProperty('skewX', makeStyleProperty('skewX', 'skewX', 0));
    animation.registerProperty('skewY', makeStyleProperty('skewY', 'skewY', 0));

    // Layout
    animation.registerProperty('left', makeStyleProperty('left', 'left', 0));
    animation.registerProperty('top', makeStyleProperty('top', 'top', 0));
    animation.registerProperty('right', makeStyleProperty('right', 'right', 0));
    animation.registerProperty('bottom', makeStyleProperty('bottom', 'bottom', 0));
    animation.registerProperty('width', makeStyleProperty('width', 'width', 0));
    animation.registerProperty('height', makeStyleProperty('height', 'height', 0));
    animation.registerProperty('borderRadius', makeStyleProperty('borderRadius', 'borderRadius', 0));
    animation.registerProperty('borderWidth', makeStyleProperty('borderWidth', 'borderWidth', 0));
    animation.registerProperty('strokeWidth', makeStyleProperty('strokeWidth', 'strokeWidth', 0));
    animation.registerProperty('strokeDasharray', makeStyleProperty('strokeDasharray', 'strokeDasharray', 0));
    animation.registerProperty('strokeDashoffset', makeStyleProperty('strokeDashoffset', 'strokeDashoffset', 0));
    animation.registerProperty('textSize', makeStyleProperty('textSize', 'textSize', 0));
    animation.registerProperty('letterSpacing', makeStyleProperty('letterSpacing', 'letterSpacing', 0));
    animation.registerProperty('lineHeight', makeStyleProperty('lineHeight', 'lineHeight', 0));
    animation.registerProperty('backdropBlur', copyOwn(makeStyleProperty('backdropBlur', 'backdropBlur', 0), { aliases: ['backdropBlurSigma'] }));
    animation.registerProperty('clipPath', {
        aliases: ['clip-path'],
        defaultValue: 'inset(0% 0% 0% 0%)',
        interpolate: interpolateClipPath,
        apply: function(target, value) {
            if (target.node) {
                target.node.clipPath(value);
                return;
            }
            if (target.set) {
                target.set({ clipPath: value });
                return;
            }
            throw new Error('target does not accept style property `clipPath`');
        },
    });
})();
