(function() {
    var runtime = globalThis.__opencatAnimation;
    var animation = runtime.animation;
    var hasOwn = runtime.hasOwn;
    var copyOwn = runtime.copyOwn;

    function resolveEasingTag(easing) {
        if (easing == null) {
            return 'linear';
        }
        if (typeof easing === 'string') {
            if (runtime.SPRING_PRESETS[easing]) {
                return runtime.SPRING_PRESETS[easing];
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
        throw new Error('invalid ease value');
    }

    function normalizeSelector(target) {
        if (typeof target !== 'string') {
            return target;
        }
        return target.charAt(0) === '#' ? target.slice(1) : target;
    }

    function normalizeTargets(targets) {
        if (targets == null) {
            return [];
        }
        if (typeof targets === 'string') {
            return [normalizeSelector(targets)];
        }
        if (Array.isArray(targets)) {
            return targets.slice().map(normalizeSelector);
        }
        if (typeof targets.set === 'function') {
            return [targets];
        }
        throw new Error('target must be an id string, array, or splitText part');
    }

    function createTarget(raw) {
        if (typeof raw === 'string') {
            return {
                kind: 'node',
                id: raw,
                raw: raw,
                node: ctx.getNode(raw),
            };
        }
        if (raw && typeof raw.set === 'function') {
            return {
                kind: 'object',
                raw: raw,
                set: function(values) {
                    raw.set(values);
                },
            };
        }
        throw new Error('invalid target');
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
                    value: Number(kf.value),
                    easing: kf.easing != null ? resolveEasingTag(kf.easing) : null,
                };
            });
            normalized.sort(function(a, b) { return a.at - b.at; });
        }
        return normalized;
    }

    function evaluateKeyframes(progress, kfs) {
        var p = Math.max(0, Math.min(1, Number(progress)));
        if (p <= kfs[0].at) return kfs[0].value;
        var last = kfs[kfs.length - 1];
        if (p >= last.at) return last.value;
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

    function splitTiming(vars, extraDelay) {
        var timing = {};
        vars = vars || {};
        for (var key in vars) {
            if (hasOwn(vars, key) && animation.isTimingKey(key)) {
                timing[key] = vars[key];
            }
        }
        if (vars.ease !== undefined && timing.easing === undefined) {
            timing.easing = vars.ease;
        }
        if (extraDelay !== undefined) {
            timing.delay = Number(timing.delay || 0) + Number(extraDelay);
        }
        return timing;
    }

    function getDescriptorDefault(descriptor, target, key, otherValue) {
        if (typeof descriptor.inferFrom === 'function') {
            return descriptor.inferFrom(otherValue, target, key);
        }
        if (hasOwn(descriptor, 'defaultValue')) {
            return typeof descriptor.defaultValue === 'function'
                ? descriptor.defaultValue(target, key, otherValue)
                : descriptor.defaultValue;
        }
        if (typeof otherValue === 'number') {
            return 0;
        }
        return otherValue;
    }

    function getVar(vars, track) {
        vars = vars || {};
        if (hasOwn(vars, track.inputName)) return vars[track.inputName];
        if (hasOwn(vars, track.name)) return vars[track.name];
        var aliases = track.descriptor.aliases || [];
        for (var i = 0; i < aliases.length; i++) {
            if (hasOwn(vars, aliases[i])) return vars[aliases[i]];
        }
        return undefined;
    }

    function resolveVarValue(value, target, targetIndex) {
        if (typeof value === 'function') {
            return value(targetIndex || 0, target ? target.raw : undefined);
        }
        return value;
    }

    function getVarForTarget(vars, track, target, targetIndex) {
        return resolveVarValue(getVar(vars, track), target, targetIndex);
    }

    function hasVar(vars, track) {
        vars = vars || {};
        if (hasOwn(vars, track.inputName) || hasOwn(vars, track.name)) return true;
        var aliases = track.descriptor.aliases || [];
        for (var i = 0; i < aliases.length; i++) {
            if (hasOwn(vars, aliases[i])) return true;
        }
        return false;
    }

    function collectTracks(fromVars, toVars, timing) {
        var tracks = [];
        var byCanonical = {};

        function add(inputName) {
            if (animation.isReservedKey(inputName)) {
                return;
            }
            var descriptor = animation.resolveProperty(inputName);
            var canonical = animation.canonicalName(inputName);
            if (!descriptor || !canonical) {
                throw new Error('unsupported animation property `' + inputName + '`');
            }
            if (byCanonical[canonical]) {
                return byCanonical[canonical];
            }
            var track = {
                inputName: inputName,
                name: canonical,
                descriptor: descriptor,
                keyframes: null,
                prepared: null,
            };
            byCanonical[canonical] = track;
            tracks.push(track);
            return track;
        }

        function addVars(vars) {
            vars = vars || {};
            for (var key in vars) {
                if (hasOwn(vars, key)) add(key);
            }
        }

        addVars(toVars);
        addVars(fromVars);

        var keyframesSpec = timing.keyframes || (toVars && toVars.keyframes) || null;
        if (keyframesSpec) {
            for (var kfKey in keyframesSpec) {
                if (hasOwn(keyframesSpec, kfKey)) {
                    add(kfKey).keyframes = normalizeKeyframes(keyframesSpec[kfKey]);
                }
            }
        }

        return {
            tracks: tracks,
            byCanonical: byCanonical,
            addTrack: add,
            hasTrack: function(name) {
                var canonical = animation.canonicalName(name) || name;
                return !!byCanonical[canonical];
            },
        };
    }

    function createTween(target, fromVars, toVars, timing, targetIndex) {
        timing = timing || {};
        fromVars = fromVars || {};
        toVars = toVars || {};

        var easingTag = resolveEasingTag(timing.ease != null ? timing.ease : timing.easing);
        var isSpring = easingTag.indexOf('spring:') === 0;
        var duration = timing.duration;
        if (duration === undefined && !isSpring) {
            throw new Error('duration is required for non-spring tweens');
        }

        var handle = __animate_create(
            duration !== undefined ? Number(duration) : -1,
            Number(timing.delay || 0),
            timing.clamp === false ? 0 : 1,
            easingTag,
            timing.repeat !== undefined ? Number(timing.repeat) : 0,
            timing.yoyo ? 1 : 0,
            timing.repeatDelay !== undefined ? Number(timing.repeatDelay) : 0
        );

        var trackSet = collectTracks(fromVars, toVars, timing);
        var tweenContext = {
            target: target,
            fromVars: fromVars,
            toVars: toVars,
            timing: timing,
            handle: handle,
            tracks: trackSet.tracks,
            addTrack: trackSet.addTrack,
            hasTrack: trackSet.hasTrack,
            sampleOverrides: {},
        };

        for (var p = 0; p < animation.plugins.length; p++) {
            var plugin = animation.plugins[p];
            if (typeof plugin.augmentTween === 'function') {
                plugin.augmentTween(tweenContext);
            }
        }

        var progress = __animate_progress(handle);
        var result = {
            progress: progress,
            settled: __animate_settled(handle),
            settleFrame: __animate_settle_frame(handle),
            values: {},
        };

        for (var i = 0; i < tweenContext.tracks.length; i++) {
            var track = tweenContext.tracks[i];
            var descriptor = track.descriptor;
            var toVal = hasVar(toVars, track)
                ? getVarForTarget(toVars, track, target, targetIndex)
                : getDescriptorDefault(
                    descriptor,
                    target,
                    track.name,
                    getVarForTarget(fromVars, track, target, targetIndex)
                );
            var fromVal = hasVar(fromVars, track)
                ? getVarForTarget(fromVars, track, target, targetIndex)
                : getDescriptorDefault(descriptor, target, track.name, toVal);

            if (track.prepared == null && typeof descriptor.prepare === 'function') {
                track.prepared = descriptor.prepare({
                    target: target,
                    from: fromVal,
                    to: toVal,
                    timing: timing,
                    vars: toVars,
                    core: runtime.core,
                });
            }

            var value;
            if (track.keyframes) {
                value = evaluateKeyframes(progress, track.keyframes);
            } else if (tweenContext.sampleOverrides[track.name]) {
                value = tweenContext.sampleOverrides[track.name]({
                    target: target,
                    from: fromVal,
                    to: toVal,
                    progress: progress,
                    handle: handle,
                    timing: timing,
                    core: runtime.core,
                });
            } else if (typeof descriptor.sample === 'function') {
                value = descriptor.sample({
                    target: target,
                    from: fromVal,
                    to: toVal,
                    progress: progress,
                    handle: handle,
                    timing: timing,
                    vars: toVars,
                    prepared: track.prepared,
                    core: runtime.core,
                });
            } else if (descriptor.interpolate === 'color') {
                value = __animate_color(handle, track.name, String(fromVal), String(toVal));
            } else if (descriptor.interpolate === 'number' || descriptor.interpolate == null) {
                value = __animate_value(handle, track.name, Number(fromVal), Number(toVal));
            } else if (typeof descriptor.interpolate === 'function') {
                value = descriptor.interpolate(fromVal, toVal, progress, {
                    target: target,
                    handle: handle,
                    timing: timing,
                    core: runtime.core,
                });
            } else {
                throw new Error(
                    'unsupported interpolation `' + descriptor.interpolate + '` for property `' + track.inputName + '`'
                );
            }

            result.values[track.inputName] = value;
            result[track.inputName] = value;
            if (track.name !== track.inputName) {
                result.values[track.name] = value;
                result[track.name] = value;
            }

            descriptor.apply(target, value, {
                inputName: track.inputName,
                name: track.name,
                timing: timing,
                vars: toVars,
                core: runtime.core,
            });
        }

        return result;
    }

    function applyTween(targets, fromVars, toVars, timing) {
        var list = normalizeTargets(targets);
        var stagger = timing && timing.stagger !== undefined ? Number(timing.stagger) : 0;
        var results = [];
        for (var i = 0; i < list.length; i++) {
            var localTiming = copyOwn(timing, {
                delay: Number((timing && timing.delay) || 0) + i * stagger,
            });
            var tween = createTween(createTarget(list[i]), fromVars, toVars, localTiming, i);
            results.push(tween);
        }
        return list.length === 1 ? results[0] : results;
    }

    runtime.core = {
        interpolate: {
            number: function(from, to, progress) {
                return Number(from) + (Number(to) - Number(from)) * Number(progress);
            },
        },
        resolveEasingTag: resolveEasingTag,
        normalizeSelector: normalizeSelector,
        normalizeTargets: normalizeTargets,
        createTarget: createTarget,
        splitTiming: splitTiming,
        getDescriptorDefault: getDescriptorDefault,
        getVar: getVar,
        collectTracks: collectTracks,
        createTween: createTween,
        applyTween: applyTween,
    };
})();
