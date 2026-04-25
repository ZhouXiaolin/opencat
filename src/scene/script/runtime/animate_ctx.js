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

    function normalizeKeyframes(spec) {
        if (!Array.isArray(spec)) {
            throw new Error('keyframes value must be an array');
        }
        if (spec.length === 0) {
            throw new Error('keyframes array cannot be empty');
        }
        var first = spec[0];
        var isShorthand = typeof first === 'number';
        var normalized;
        if (isShorthand) {
            var n = spec.length;
            normalized = [];
            for (var i = 0; i < n; i++) {
                normalized.push({
                    at: n === 1 ? 0 : i / (n - 1),
                    value: Number(spec[i]),
                    easing: null,
                });
            }
        } else {
            normalized = spec.map(function(kf) {
                if (kf == null || typeof kf.at !== 'number' || typeof kf.value !== 'number') {
                    throw new Error('keyframe entry requires numeric `at` and `value`');
                }
                return {
                    at: kf.at,
                    value: kf.value,
                    easing: kf.easing != null ? resolveEasingTag(kf.easing) : null,
                };
            });
            normalized.sort(function(a, b) { return a.at - b.at; });
        }
        return normalized;
    }

    function evaluateKeyframes(progress, kfs) {
        var p = Math.max(0, Math.min(1, Number(progress)));
        if (p <= kfs[0].at) { return kfs[0].value; }
        var last = kfs[kfs.length - 1];
        if (p >= last.at) { return last.value; }
        for (var i = 0; i < kfs.length - 1; i++) {
            var a = kfs[i];
            var b = kfs[i + 1];
            if (p >= a.at && p <= b.at) {
                var span = b.at - a.at;
                var localT = span > 0 ? (p - a.at) / span : 0;
                if (b.easing) {
                    localT = __easing_apply(b.easing, localT);
                }
                return a.value + (b.value - a.value) * localT;
            }
        }
        return last.value;
    }

    var ANIM_DEFAULTS = {
        'opacity': 1,
        'translateX': 0,
        'translateY': 0,
        'scale': 1,
        'scaleX': 1,
        'scaleY': 1,
        'rotation': 0,
        'skewX': 0,
        'skewY': 0,
        'x': 0,
        'y': 0,
        'left': 0,
        'top': 0,
        'right': 0,
        'bottom': 0,
        'width': 0,
        'height': 0,
    };

    var ANIM_KEY_TO_SETTER = {
        'opacity': 'opacity',
        'translateX': 'translateX',
        'translateY': 'translateY',
        'scale': 'scale',
        'scaleX': 'scaleX',
        'scaleY': 'scaleY',
        'rotation': 'rotate',
        'skewX': 'skewX',
        'skewY': 'skewY',
        'x': 'left',
        'y': 'top',
        'left': 'left',
        'top': 'top',
        'right': 'right',
        'bottom': 'bottom',
        'width': 'width',
        'height': 'height',
        'bg': 'bg',
        'textColor': 'textColor',
        'borderRadius': 'borderRadius',
        'borderWidth': 'borderWidth',
        'borderColor': 'borderColor',
    };

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
        var stagger = opts.stagger || 0;

        if (duration === undefined && !isSpring) {
            throw new Error('duration is required for non-spring easing');
        }

        // GSAP-like from-only semantics: if only `from` is given, infer `to`
        // from identity defaults so the animation plays from the given state
        // back to the natural resting state.
        var fromKeys = Object.keys(from);
        var toKeys = Object.keys(to);
        var allKeys = fromKeys.slice();
        for (var ti = 0; ti < toKeys.length; ti++) {
            if (allKeys.indexOf(toKeys[ti]) === -1) {
                allKeys.push(toKeys[ti]);
            }
        }
        for (var ki = 0; ki < allKeys.length; ki++) {
            var key = allKeys[ki];
            if (!(key in to)) {
                to[key] = ANIM_DEFAULTS[key] !== undefined ? ANIM_DEFAULTS[key] : 0;
            }
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
            stagger: stagger,
            targets: opts.targets || null,
        };
    }

    function normalizeTargets(targets) {
        if (typeof targets === 'string') return [targets];
        if (Array.isArray(targets)) return targets.slice();
        if (targets && typeof targets.set === 'function') return [targets];
        return [];
    }

    function applyTargets(targets, keys, result) {
        var list = normalizeTargets(targets);
        for (var i = 0; i < list.length; i++) {
            var target = list[i];
            var values = {};
            for (var k = 0; k < keys.length; k++) {
                var key = keys[k];
                var setterKey = ANIM_KEY_TO_SETTER[key] || key;
                values[setterKey] = result[key];
            }
            if (typeof target.set === 'function') {
                target.set(values);
            } else if (typeof target === 'string') {
                var node = ctx.getNode(target);
                for (var sk in values) {
                    if (values[sk] !== undefined) {
                        node[sk](values[sk]);
                    }
                }
            }
        }
    }

    ctx.animate = function(opts) {
        var parsed = parseAnimateOptions(opts);

        var pathHandle = null;
        var pathOrient = 0;
        if (opts.path) {
            var svg = String(opts.path);
            pathOrient = opts.orient !== undefined ? Number(opts.orient) : 0;
            var cacheKey = '__ap_' + svg;
            if (!ctx[cacheKey]) {
                ctx[cacheKey] = __along_path_create(svg);
            }
            pathHandle = ctx[cacheKey];
        }

        var keyframesSpec = opts.keyframes || null;
        var keyframesNormalized = null;
        if (keyframesSpec) {
            keyframesNormalized = {};
            for (var kfKey in keyframesSpec) {
                if (Object.prototype.hasOwnProperty.call(keyframesSpec, kfKey)) {
                    keyframesNormalized[kfKey] = normalizeKeyframes(keyframesSpec[kfKey]);
                }
            }
        }

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
        if (keyframesNormalized) {
            for (var kfKey2 in keyframesNormalized) {
                if (Object.prototype.hasOwnProperty.call(keyframesNormalized, kfKey2)) {
                    (function(key, kfs) {
                        Object.defineProperty(result, key, {
                            get: function() {
                                var p = __animate_progress(handle);
                                return evaluateKeyframes(p, kfs);
                            },
                            enumerable: true,
                        });
                    })(kfKey2, keyframesNormalized[kfKey2]);
                }
            }
        }
        if (pathHandle != null) {
            var _sp = -1, _sa = null;
            function _pathSample() {
                var p = __animate_progress(handle);
                if (p !== _sp) { _sa = __along_path_at(pathHandle, p); _sp = p; }
                return _sa;
            }
            Object.defineProperty(result, 'x', {
                get: function() { return _pathSample()[0]; },
                enumerable: true,
            });
            Object.defineProperty(result, 'y', {
                get: function() { return _pathSample()[1]; },
                enumerable: true,
            });
            Object.defineProperty(result, 'rotation', {
                get: function() { return _pathSample()[2] + pathOrient; },
                enumerable: true,
            });
        }
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

        if (parsed.targets) {
            applyTargets(parsed.targets, keys, result);
        }

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

        var targetList = normalizeTargets(parsed.targets);
        if (targetList.length > 0) {
            count = targetList.length;
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

        if (targetList.length > 0) {
            for (var si = 0; si < targetList.length && si < results.length; si++) {
                applyTargets([targetList[si]], keys, results[si]);
            }
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
        random: function(min, max, seed) {
            var lo = Number(min);
            var hi = Number(max);
            if (seed === undefined || seed === null) {
                return lo + Math.random() * (hi - lo);
            }
            return lo + __util_random_seeded(Number(seed)) * (hi - lo);
        },
        randomInt: function(min, max, seed) {
            return Math.floor(this.random(Number(min), Number(max) + 1, seed));
        },
    };

    ctx.alongPath = function(svgPath) {
        var svg = String(svgPath);
        var handle = __along_path_create(svg);
        var len = __along_path_length(handle);
        return {
            getLength: function() { return len; },
            at: function(t) {
                if (handle == null) {
                    throw new Error('alongPath result has been disposed');
                }
                var arr = __along_path_at(handle, Number(t));
                return { x: arr[0], y: arr[1], angle: arr[2] };
            },
            dispose: function() {
                if (handle != null) {
                    __along_path_dispose(handle);
                    handle = null;
                }
            },
        };
    };

    ctx.splitTextNode = function(nodeId, opts) {
        if (!nodeId) {
            throw new Error("ctx.splitTextNode requires a node id");
        }
        var options = opts || {};
        var granularity = options.granularity || "graphemes";
        var text = __text_source_get(String(nodeId));
        if (typeof text !== "string") {
            throw new Error("no resolved text source for node `" + nodeId + "`");
        }

        var parts = __text_units_describe(String(nodeId), granularity);
        var partsArray = parts.map(function(meta) {
            var part = {
                index: meta[0],
                text: meta[1],
                start: meta[2],
                end: meta[3],
                set: function(values) {
                    values = values || {};
                    __record_text_unit_override(
                        String(nodeId),
                        granularity,
                        meta[0],
                        values
                    );
                    return part;
                }
            };
            return part;
        });

        partsArray.animate = function(animOpts) {
            animOpts = animOpts || {};
            var anims = ctx.stagger(partsArray.length, {
                from: animOpts.from,
                to: animOpts.to,
                duration: animOpts.duration,
                delay: animOpts.delay,
                gap: animOpts.stagger !== undefined ? animOpts.stagger : (animOpts.gap || 0),
                easing: animOpts.easing,
                clamp: animOpts.clamp,
                repeat: animOpts.repeat,
                yoyo: animOpts.yoyo,
                repeatDelay: animOpts.repeatDelay,
                targets: partsArray
            });
            return {
                length: partsArray.length,
                values: anims
            };
        };

        partsArray.revert = function() {
            for (var i = 0; i < partsArray.length; i++) {
                partsArray[i].set({
                    opacity: 1,
                    translateX: 0,
                    translateY: 0,
                    scale: 1,
                    rotation: 0
                });
            }
            return partsArray;
        };

        return partsArray;
    };
})();
