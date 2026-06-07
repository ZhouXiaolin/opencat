(function() {
    var animation = globalThis.__opencatAnimation.animation;

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

    animation.registerProperty('backgroundColor', makeColorProperty('bg', ['bg']));
    animation.registerProperty('textColor', makeColorProperty('textColor', ['color']));
    animation.registerProperty('borderColor', makeColorProperty('borderColor'));
    animation.registerProperty('fillColor', makeColorProperty('fillColor'));
    animation.registerProperty('strokeColor', makeColorProperty('strokeColor'));
})();
