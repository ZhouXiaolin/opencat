(function() {
    var runtime = globalThis.__opencatAnimation;

    function sampleText(fromText, toText, progress, mode) {
        var from = fromText == null ? '' : String(fromText);
        var to = toText == null ? '' : String(toText);
        var textMode = mode || 'typewriter';
        if (progress <= 0) return from;
        if (progress >= 1) return to;
        if (textMode !== 'typewriter') {
            throw new Error('unsupported text animation mode `' + textMode + '`');
        }
        var parts = __text_graphemes(to);
        var n = Math.floor(parts.length * progress);
        return parts.slice(0, n).join('');
    }

    runtime.animation.registerPlugin({
        name: 'text',
        specialOptions: ['mode', 'textMode'],
        properties: {
            text: {
                defaultValue: '',
                sample: function(sample) {
                    return sampleText(
                        sample.from,
                        sample.to,
                        sample.progress,
                        sample.timing.mode || sample.timing.textMode || sample.vars && (sample.vars.mode || sample.vars.textMode)
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
