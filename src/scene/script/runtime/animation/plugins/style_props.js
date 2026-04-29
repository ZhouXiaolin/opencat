(function() {
    var runtime = globalThis.__opencatAnimation;
    var copyOwn = runtime.copyOwn;

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

    runtime.animation.registerPlugin({
        name: 'style-props',
        properties: {
            opacity: makeStyleProperty('opacity', 'opacity', 1),
            x: copyOwn(makeStyleProperty('translateX', 'x', 0), { aliases: ['translateX'] }),
            y: copyOwn(makeStyleProperty('translateY', 'y', 0), { aliases: ['translateY'] }),
            scale: makeStyleProperty('scale', 'scale', 1),
            scaleX: makeStyleProperty('scaleX', 'scaleX', 1),
            scaleY: makeStyleProperty('scaleY', 'scaleY', 1),
            rotation: copyOwn(makeStyleProperty('rotate', 'rotation', 0), { aliases: ['rotate'] }),
            skewX: makeStyleProperty('skewX', 'skewX', 0),
            skewY: makeStyleProperty('skewY', 'skewY', 0),
            left: makeStyleProperty('left', 'left', 0),
            top: makeStyleProperty('top', 'top', 0),
            right: makeStyleProperty('right', 'right', 0),
            bottom: makeStyleProperty('bottom', 'bottom', 0),
            width: makeStyleProperty('width', 'width', 0),
            height: makeStyleProperty('height', 'height', 0),
            borderRadius: makeStyleProperty('borderRadius', 'borderRadius', 0),
            borderWidth: makeStyleProperty('borderWidth', 'borderWidth', 0),
            strokeWidth: makeStyleProperty('strokeWidth', 'strokeWidth', 0),
            textSize: makeStyleProperty('textSize', 'textSize', 0),
            letterSpacing: makeStyleProperty('letterSpacing', 'letterSpacing', 0),
            lineHeight: makeStyleProperty('lineHeight', 'lineHeight', 0),
        },
    });
})();
