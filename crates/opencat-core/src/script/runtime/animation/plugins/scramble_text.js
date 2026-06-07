(function() {
    var runtime = globalThis.__opencatAnimation;

    var DEFAULT_CHARS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789';
    var SYMBOL_CHARS = '!@#$%^&*()_+-=[]{}|;:,.<>?';

    function charsFor(value) {
        if (value == null || value === true) return DEFAULT_CHARS;
        if (value === false) return '';
        var name = String(value);
        switch (name) {
            case 'upperCase':
            case 'uppercase':
                return 'ABCDEFGHIJKLMNOPQRSTUVWXYZ';
            case 'lowerCase':
            case 'lowercase':
                return 'abcdefghijklmnopqrstuvwxyz';
            case 'upperAndLowerCase':
            case 'letters':
                return 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz';
            case 'numbers':
            case 'digits':
                return '0123456789';
            case 'symbols':
                return SYMBOL_CHARS;
            case 'all':
                return DEFAULT_CHARS + 'abcdefghijklmnopqrstuvwxyz' + SYMBOL_CHARS;
            default:
                return name;
        }
    }

    function hasOwn(obj, key) {
        return Object.prototype.hasOwnProperty.call(obj, key);
    }

    function option(spec, vars, name, fallback) {
        if (spec && typeof spec === 'object' && hasOwn(spec, name)) return spec[name];
        if (vars && hasOwn(vars, name)) return vars[name];
        return fallback;
    }

    function targetText(value, vars) {
        if (value && typeof value === 'object' && hasOwn(value, 'text')) {
            return value.text == null ? '' : String(value.text);
        }
        if (value === true && vars && hasOwn(vars, 'text')) {
            return vars.text == null ? '' : String(vars.text);
        }
        if (value == null || value === true) return '';
        return String(value);
    }

    function splitText(text, delimiter) {
        text = text == null ? '' : String(text);
        if (delimiter != null) {
            return text.split(String(delimiter));
        }
        return __text_graphemes(text);
    }

    function joinText(parts, delimiter) {
        return parts.join(delimiter != null ? String(delimiter) : '');
    }

    function hashString(value) {
        var hash = 0;
        var s = String(value || '');
        for (var i = 0; i < s.length; i++) {
            hash = ((hash << 5) - hash + s.charCodeAt(i)) | 0;
        }
        return Math.abs(hash);
    }

    function randomChar(chars, index, tick, targetId) {
        if (!chars) return '';
        var seed = (index + 1) * 12.9898 + (tick + 1) * 78.233 + hashString(targetId) * 0.013;
        var n = Math.floor(__util_random_seeded(seed) * chars.length);
        return chars.charAt(Math.max(0, Math.min(chars.length - 1, n)));
    }

    function isWhitespace(value) {
        return typeof value === 'string' && /^\s$/.test(value);
    }

    function sampleScramble(sample) {
        var vars = sample.vars || {};
        var spec = sample.to;
        var fromText = targetText(sample.from, vars);
        var toText = targetText(spec, vars);
        var delimiter = option(spec, vars, 'delimiter', null);
        var chars = charsFor(option(spec, vars, 'chars', DEFAULT_CHARS));
        var revealDelay = Math.max(0, Math.min(1, Number(option(spec, vars, 'revealDelay', 0) || 0)));
        var tweenLength = option(spec, vars, 'tweenLength', true) !== false;
        var rightToLeft = !!option(spec, vars, 'rightToLeft', false);
        var speed = Math.max(0.001, Number(option(spec, vars, 'speed', 20) || 20));
        var progress = Math.max(0, Math.min(1, Number(sample.progress || 0)));

        if (progress >= 1) return toText;

        var fromParts = splitText(fromText, delimiter);
        var toParts = splitText(toText, delimiter);
        var fromLen = fromParts.length;
        var toLen = toParts.length;
        var length = tweenLength
            ? Math.round(fromLen + (toLen - fromLen) * progress)
            : fromLen;
        length = Math.max(0, length);

        var revealProgress = progress <= revealDelay
            ? 0
            : (progress - revealDelay) / Math.max(0.0001, 1 - revealDelay);
        var revealed = Math.floor(toLen * Math.max(0, Math.min(1, revealProgress)));
        var tick = Math.floor(Number(ctx.currentTime || 0) * speed);
        var targetId = sample.target && sample.target.id ? sample.target.id : '';
        var out = [];

        for (var i = 0; i < length; i++) {
            var targetIndex = i;
            var visible = rightToLeft
                ? targetIndex >= Math.max(0, toLen - revealed)
                : targetIndex < revealed;
            if (visible && targetIndex < toLen) {
                out.push(toParts[targetIndex]);
                continue;
            }

            var fixed = targetIndex < toLen ? toParts[targetIndex] : fromParts[targetIndex];
            if (delimiter == null && isWhitespace(fixed)) {
                out.push(fixed);
            } else {
                out.push(randomChar(chars, rightToLeft ? length - i - 1 : i, tick, targetId));
            }
        }

        return joinText(out, delimiter);
    }

    runtime.animation.registerPlugin({
        name: 'scramble-text',
        specialOptions: [
            'chars',
            'speed',
            'revealDelay',
            'tweenLength',
            'delimiter',
            'rightToLeft',
            'newClass',
            'oldClass',
        ],
        properties: {
            scrambleText: {
                defaultValue: function(target) {
                    if (target && target.id) {
                        var text = __text_source_get(target.id);
                        if (typeof text === 'string') return text;
                    }
                    return '';
                },
                sample: sampleScramble,
                apply: function(target, value) {
                    if (!target.node) {
                        throw new Error('scrambleText animation requires a node target');
                    }
                    target.node.text(value);
                },
            },
        },
    });
})();
