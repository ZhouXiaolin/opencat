(function() {
    var runtime = globalThis.__opencatAnimation;

    function makeColorProperty(nodeSetter, aliases) {
        return {
            aliases: aliases || [],
            interpolate: 'color',
            inferFrom: function(toValue) {
                return toValue;
            },
            apply: function(target, value) {
                if (target.node) {
                    target.node[nodeSetter](value);
                    return;
                }
                if (target.set) {
                    var values = {};
                    values[aliases && aliases.indexOf('color') !== -1 ? 'color' : nodeSetter] = value;
                    target.set(values);
                    return;
                }
                throw new Error('color animation requires a node target');
            },
        };
    }

    runtime.animation.registerPlugin({
        name: 'color',
        properties: {
            backgroundColor: makeColorProperty('bg', ['bg']),
            textColor: makeColorProperty('textColor', ['color']),
            borderColor: makeColorProperty('borderColor'),
            fillColor: makeColorProperty('fillColor'),
            strokeColor: makeColorProperty('strokeColor'),
        },
    });
})();
