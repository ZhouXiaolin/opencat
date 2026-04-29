(function() {
    var runtime = globalThis.__opencatAnimation;
    var core = runtime.core;
    var hasOwn = runtime.hasOwn;

    runtime.animation.registerPlugin({
        name: 'split-text',
        install: function() {
            ctx.splitText = function(target, opts) {
                var id = core.normalizeSelector(target);
                if (typeof id !== 'string' || !id) {
                    throw new Error('ctx.splitText requires a node id');
                }
                var options = opts || {};
                var type = options.type || options.granularity || 'chars';
                var granularity;
                if (type === 'chars' || type === 'characters' || type === 'graphemes') {
                    granularity = 'graphemes';
                } else if (type === 'words') {
                    granularity = 'words';
                } else if (type === 'lines') {
                    throw new Error('splitText({ type: "lines" }) requires layout-derived line ranges and is not implemented yet');
                } else {
                    throw new Error('unsupported splitText type `' + type + '`');
                }

                var text = __text_source_get(id);
                if (typeof text !== 'string') {
                    throw new Error('no resolved text source for node `' + id + '`');
                }

                var parts = __text_units_describe(id, granularity).map(function(meta) {
                    var part = {
                        index: meta[0],
                        text: meta[1],
                        start: meta[2],
                        end: meta[3],
                        set: function(values) {
                            values = values || {};
                            var mapped = {};
                            for (var key in values) {
                                if (!hasOwn(values, key)) continue;
                                switch (key) {
                                    case 'opacity': mapped.opacity = values[key]; break;
                                    case 'x':
                                    case 'translateX': mapped.translateX = values[key]; break;
                                    case 'y':
                                    case 'translateY': mapped.translateY = values[key]; break;
                                    case 'scale': mapped.scale = values[key]; break;
                                    case 'rotate':
                                    case 'rotation': mapped.rotation = values[key]; break;
                                    default:
                                        throw new Error('unsupported splitText property `' + key + '`');
                                }
                            }
                            __record_text_unit_override(id, granularity, meta[0], mapped);
                            return part;
                        }
                    };
                    return part;
                });

                parts.revert = function() {
                    for (var i = 0; i < parts.length; i++) {
                        parts[i].set({ opacity: 1, x: 0, y: 0, scale: 1, rotate: 0 });
                    }
                    return parts;
                };

                return parts;
            };
        },
    });
})();
