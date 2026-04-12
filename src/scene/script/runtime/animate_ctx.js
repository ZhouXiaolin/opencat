(function() {
    var SPRING_PRESETS = {
        'spring-default':  'spring:100,10,1',
        'spring-gentle':   'spring:60,8,0.8',
        'spring-stiff':    'spring:200,15,1',
        'spring-slow':     'spring:80,12,1.5',
        'spring-wobbly':   'spring:180,6,1',
    };

    function resolveEasingTag(easing) {
        if (typeof easing === 'string') {
            if (SPRING_PRESETS[easing]) {
                return SPRING_PRESETS[easing];
            }
            return easing;
        }
        if (Array.isArray(easing) && easing.length === 4) {
            return 'bezier:' + easing[0] + ',' + easing[1] + ',' + easing[2] + ',' + easing[3];
        }
        if (easing && typeof easing === 'object' && easing.spring) {
            var s = easing.spring;
            return 'spring:' + s.stiffness + ',' + s.damping + ',' + s.mass;
        }
        return 'linear';
    }

    function parseAnimateOptions(opts) {
        var from = opts.from || {};
        var to = opts.to || {};
        var duration = opts.duration;
        var delay = opts.delay || 0;
        var easingTag = resolveEasingTag(opts.easing || 'linear');
        var isSpring = easingTag.indexOf('spring:') === 0;
        var clamp = opts.clamp || false;

        if (duration === undefined && !isSpring) {
            throw new Error('duration is required for non-spring easing');
        }

        return {
            from: from,
            to: to,
            duration: duration !== undefined ? duration : -1,
            delay: delay,
            easingTag: easingTag,
            clamp: clamp,
        };
    }

    ctx.animate = function(opts) {
        var parsed = parseAnimateOptions(opts);

        var keys = Object.keys(parsed.from);
        var toKeys = Object.keys(parsed.to);
        for (var ti = 0; ti < toKeys.length; ti++) {
            if (!(toKeys[ti] in parsed.from)) {
                keys.push(toKeys[ti]);
            }
        }

        var handle = __animate_create(
            parsed.duration,
            parsed.delay,
            parsed.clamp ? 1 : 0,
            parsed.easingTag
        );

        var result = {};
        for (var ki = 0; ki < keys.length; ki++) {
            (function(key) {
                var fromVal = parsed.from[key] !== undefined ? parsed.from[key] : 0;
                var toVal = parsed.to[key] !== undefined ? parsed.to[key] : fromVal;
                var isColor = typeof fromVal === 'string' || typeof toVal === 'string';

                if (isColor) {
                    Object.defineProperty(result, key, {
                        get: function() {
                            return __animate_color(handle, key, String(fromVal), String(toVal));
                        },
                        enumerable: true,
                    });
                } else {
                    Object.defineProperty(result, key, {
                        get: function() {
                            return __animate_value(handle, key, Number(fromVal), Number(toVal));
                        },
                        enumerable: true,
                    });
                }
            })(keys[ki]);
        }

        Object.defineProperty(result, 'progress', {
            get: function() { return __animate_progress(handle); },
            enumerable: true,
        });

        Object.defineProperty(result, 'settled', {
            get: function() { return __animate_settled(handle); },
            enumerable: true,
        });

        Object.defineProperty(result, 'settleFrame', {
            get: function() { return __animate_settle_frame(handle); },
            enumerable: true,
        });

        return result;
    };

    ctx.stagger = function(count, opts) {
        var parsed = parseAnimateOptions(opts);
        var gap = opts.gap || 0;

        var keys = Object.keys(parsed.from);
        var toKeys = Object.keys(parsed.to);
        for (var ti = 0; ti < toKeys.length; ti++) {
            if (!(toKeys[ti] in parsed.from)) {
                keys.push(toKeys[ti]);
            }
        }

        var results = [];
        for (var i = 0; i < count; i++) {
            (function(index) {
                var handle = __animate_create(
                    parsed.duration,
                    parsed.delay + index * gap,
                    parsed.clamp ? 1 : 0,
                    parsed.easingTag
                );

                var item = {};
                for (var ki = 0; ki < keys.length; ki++) {
                    (function(key) {
                        var fromVal = parsed.from[key] !== undefined ? parsed.from[key] : 0;
                        var toVal = parsed.to[key] !== undefined ? parsed.to[key] : fromVal;
                        var isColor = typeof fromVal === 'string' || typeof toVal === 'string';

                        if (isColor) {
                            Object.defineProperty(item, key, {
                                get: function() {
                                    return __animate_color(handle, key, String(fromVal), String(toVal));
                                },
                                enumerable: true,
                            });
                        } else {
                            Object.defineProperty(item, key, {
                                get: function() {
                                    return __animate_value(handle, key, Number(fromVal), Number(toVal));
                                },
                                enumerable: true,
                            });
                        }
                    })(keys[ki]);
                }

                Object.defineProperty(item, 'progress', {
                    get: function() { return __animate_progress(handle); },
                    enumerable: true,
                });

                Object.defineProperty(item, 'settled', {
                    get: function() { return __animate_settled(handle); },
                    enumerable: true,
                });

                Object.defineProperty(item, 'settleFrame', {
                    get: function() { return __animate_settle_frame(handle); },
                    enumerable: true,
                });

                results.push(item);
            })(i);
        }

        return results;
    };
})();
