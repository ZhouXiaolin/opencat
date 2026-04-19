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
        var repeat = opts.repeat !== undefined ? Number(opts.repeat) : 0;
        var yoyo = !!opts.yoyo;
        var repeatDelay = opts.repeatDelay !== undefined ? Number(opts.repeatDelay) : 0;

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
            repeat: repeat,
            yoyo: yoyo,
            repeatDelay: repeatDelay,
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
            parsed.easingTag,
            parsed.repeat,
            parsed.yoyo ? 1 : 0,
            parsed.repeatDelay
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
                    parsed.easingTag,
                    parsed.repeat,
                    parsed.yoyo ? 1 : 0,
                    parsed.repeatDelay
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

    ctx.sequence = function(steps) {
        if (!Array.isArray(steps)) {
            throw new Error('ctx.sequence expects an array of steps');
        }

        var results = [];
        var cursor = 0;

        for (var si = 0; si < steps.length; si++) {
            (function(step) {
                var parsed = parseAnimateOptions(step);
                var hasExplicitAt = step.at !== undefined && step.at !== null;
                var startFrame = hasExplicitAt
                    ? Number(step.at)
                    : cursor + parsed.delay;

                var handle = __animate_create(
                    parsed.duration,
                    startFrame,
                    parsed.clamp ? 1 : 0,
                    parsed.easingTag,
                    parsed.repeat,
                    parsed.yoyo ? 1 : 0,
                    parsed.repeatDelay
                );

                var keys = Object.keys(parsed.from);
                var toKeys = Object.keys(parsed.to);
                for (var ti = 0; ti < toKeys.length; ti++) {
                    if (!(toKeys[ti] in parsed.from)) {
                        keys.push(toKeys[ti]);
                    }
                }

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

                if (!hasExplicitAt) {
                    var endFrame = __animate_settle_frame(handle);
                    var gap = step.gap !== undefined ? Number(step.gap) : 0;
                    cursor = endFrame + gap;
                }

                results.push(item);
            })(steps[si]);
        }

        return results;
    };

    ctx.typewriter = function(fullText, opts) {
        if (!opts) {
            throw new Error('ctx.typewriter requires an options object');
        }
        var full = String(fullText);
        var chars = Array.from(full);
        var total = chars.length;
        var caret = opts.caret !== undefined ? String(opts.caret) : '';

        var anim = ctx.animate({
            from: { chars: 0 },
            to:   { chars: total },
            duration: opts.duration,
            delay: opts.delay || 0,
            easing: opts.easing || 'linear',
            clamp: opts.clamp !== undefined ? opts.clamp : true,
        });

        var result = {};
        Object.defineProperty(result, 'text', {
            get: function() {
                var n = Math.floor(anim.chars);
                if (n <= 0) return '';
                if (n >= total) return full;
                return chars.slice(0, n).join('') + caret;
            },
            enumerable: true,
        });
        Object.defineProperty(result, 'progress', {
            get: function() { return anim.progress; },
            enumerable: true,
        });
        Object.defineProperty(result, 'settled', {
            get: function() { return anim.settled; },
            enumerable: true,
        });
        Object.defineProperty(result, 'settleFrame', {
            get: function() { return anim.settleFrame; },
            enumerable: true,
        });

        return result;
    };

    ctx.utils = {
        clamp: function(value, min, max) {
            return Math.min(max, Math.max(min, Number(value)));
        },
        snap: function(value, step) {
            var n = Number(value);
            var s = Number(step);
            if (!isFinite(s) || s <= 0) { return n; }
            return Math.round(n / s) * s;
        },
        wrap: function(value, min, max) {
            var lo = Number(min);
            var hi = Number(max);
            var range = hi - lo;
            if (range <= 0) { return lo; }
            var n = (Number(value) - lo) % range;
            if (n < 0) { n += range; }
            return n + lo;
        },
        mapRange: function(value, inMin, inMax, outMin, outMax) {
            var iMin = Number(inMin);
            var iMax = Number(inMax);
            if (iMax === iMin) { return Number(outMin); }
            var t = (Number(value) - iMin) / (iMax - iMin);
            return Number(outMin) + t * (Number(outMax) - Number(outMin));
        },
    };
})();
