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
})();
