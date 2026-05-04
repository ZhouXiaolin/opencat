(function() {
    var runtime = globalThis.__opencatAnimation;

    function cursorText(vars, timing) {
        if (vars && vars.cursor != null) return String(vars.cursor);
        if (timing && timing.cursor != null) return String(timing.cursor);
        if (vars && vars.typewriterCursor != null) return String(vars.typewriterCursor);
        if (timing && timing.typewriterCursor != null) return String(timing.typewriterCursor);
        return '';
    }

    function cursorVisible(progress, vars, timing) {
        var cursor = cursorText(vars, timing);
        if (!cursor || progress >= 1) return false;
        var blink = vars && vars.cursorBlink != null ? vars.cursorBlink : timing && timing.cursorBlink;
        if (blink === false) return true;
        var period = Number(blink || 12);
        return Math.floor(ctx.currentFrame / Math.max(1, period)) % 2 === 0;
    }

    function sampleText(fromText, toText, progress, mode, vars, timing) {
        var from = fromText == null ? '' : String(fromText);
        var to = toText == null ? '' : String(toText);
        var textMode = mode || 'typewriter';
        if (textMode !== 'typewriter') {
            throw new Error('unsupported text animation mode `' + textMode + '`');
        }
        if (progress <= 0) {
            return from + (cursorVisible(progress, vars, timing) ? cursorText(vars, timing) : '');
        }
        if (progress >= 1) return to;
        var parts = __text_graphemes(to);
        var n = Math.floor(parts.length * progress);
        var sampled = parts.slice(0, n).join('');
        return sampled + (cursorVisible(progress, vars, timing) ? cursorText(vars, timing) : '');
    }

    runtime.animation.registerPlugin({
        name: 'text',
        specialOptions: ['mode', 'textMode', 'cursor', 'typewriterCursor', 'cursorBlink'],
        properties: {
            text: {
                defaultValue: '',
                sample: function(sample) {
                    return sampleText(
                        sample.from,
                        sample.to,
                        sample.progress,
                        sample.timing.mode || sample.timing.textMode || sample.vars && (sample.vars.mode || sample.vars.textMode),
                        sample.vars,
                        sample.timing
                    );
                },
                apply: function(target, value) {
                    if (!target.node) {
                        throw new Error('text animation requires a node target');
                    }
                    target.node.text(value);
                },
            },
        },
    });
})();
