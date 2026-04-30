(function() {
    var runtime = globalThis.__opencatAnimation;
    var core = runtime.core;
    var copyOwn = runtime.copyOwn;
    var hasOwn = runtime.hasOwn;

    function parsePosition(pos, state) {
        if (pos == null) return state.cursor;
        if (typeof pos === 'number') return pos;
        if (typeof pos === 'string') {
            var prevStart = state.previousStart == null ? state.cursor : state.previousStart;
            var prevEnd = state.previousEnd == null ? state.cursor : state.previousEnd;

            if (pos === '<') return prevStart;
            if (pos === '>') return prevEnd;

            var relativeToPrevious = pos.match(/^([<>])([+-]=)(-?\d+(?:\.\d+)?)$/);
            if (relativeToPrevious) {
                var base = relativeToPrevious[1] === '<' ? prevStart : prevEnd;
                var amount = Number(relativeToPrevious[3]);
                return relativeToPrevious[2] === '+=' ? base + amount : base - amount;
            }

            if (pos.indexOf('+=') === 0) return state.cursor + Number(pos.slice(2));
            if (pos.indexOf('-=') === 0) return state.cursor - Number(pos.slice(2));

            var labelRelative = pos.match(/^(.+?)([+-]=)(-?\d+(?:\.\d+)?)$/);
            if (labelRelative && hasOwn(state.labels, labelRelative[1])) {
                var labelBase = state.labels[labelRelative[1]];
                var labelAmount = Number(labelRelative[3]);
                return labelRelative[2] === '+=' ? labelBase + labelAmount : labelBase - labelAmount;
            }

            if (hasOwn(state.labels, pos)) return state.labels[pos];
            var n = Number(pos);
            if (!isNaN(n)) return n;
        }
        throw new Error('unsupported timeline position `' + pos + '`');
    }

    ctx.set = function(targets, vars) {
        var list = core.normalizeTargets(targets);
        vars = vars || {};
        var trackSet = core.collectTracks({}, vars, {});
        for (var i = 0; i < list.length; i++) {
            var target = core.createTarget(list[i]);
            for (var t = 0; t < trackSet.tracks.length; t++) {
                var track = trackSet.tracks[t];
                var value = core.getVar(vars, track);
                track.descriptor.apply(target, value, {
                    inputName: track.inputName,
                    name: track.name,
                    timing: {},
                    vars: vars,
                    core: core,
                });
            }
        }
        return list;
    };

    ctx.to = function(targets, vars) {
        return core.applyTween(targets, {}, vars || {}, core.splitTiming(vars));
    };

    ctx.from = function(targets, vars) {
        vars = vars || {};
        var toVars = {};
        var tracks = core.collectTracks(vars, {}, {}).tracks;
        for (var i = 0; i < tracks.length; i++) {
            var track = tracks[i];
            toVars[track.inputName] = core.getDescriptorDefault(
                track.descriptor,
                null,
                track.name,
                core.getVar(vars, track)
            );
        }
        return core.applyTween(targets, vars, toVars, core.splitTiming(vars));
    };

    ctx.fromTo = function(targets, fromVars, toVars) {
        return core.applyTween(targets, fromVars || {}, toVars || {}, core.splitTiming(toVars));
    };

    ctx.timeline = function(opts) {
        opts = opts || {};
        var baseDelay = Number(opts.delay || 0);
        var defaults = opts.defaults || {};
        var state = {
            labels: {},
            cursor: 0,
            previousStart: null,
            previousEnd: null,
        };

        function recordChild(start, duration) {
            state.previousStart = start;
            state.previousEnd = start + duration;
        }

        function addTween(kind, targets, a, b, pos) {
            var fromVars;
            var toVars;
            if (kind === 'fromTo') {
                fromVars = a || {};
                toVars = b || {};
            } else if (kind === 'from') {
                fromVars = a || {};
                toVars = {};
                var tracks = core.collectTracks(fromVars, {}, {}).tracks;
                for (var fi = 0; fi < tracks.length; fi++) {
                    var track = tracks[fi];
                    toVars[track.inputName] = core.getDescriptorDefault(
                        track.descriptor,
                        null,
                        track.name,
                        core.getVar(fromVars, track)
                    );
                }
            } else {
                fromVars = {};
                toVars = a || {};
            }

            var varsForTiming = kind === 'fromTo' ? toVars : a;
            var hasExplicitPosition = pos != null || (varsForTiming && varsForTiming.at != null);
            var positionValue = hasExplicitPosition ? (pos != null ? pos : varsForTiming.at) : null;
            var start = parsePosition(positionValue, state);
            var mergedTiming = core.splitTiming(copyOwn(defaults, varsForTiming), baseDelay + start);
            var duration = mergedTiming.duration !== undefined ? Number(mergedTiming.duration) : 0;
            if (!hasExplicitPosition) {
                state.cursor = Math.max(state.cursor, start + duration);
            }
            recordChild(start, duration);
            if (ctx.currentFrame < baseDelay + start) {
                return null;
            }
            return core.applyTween(targets, fromVars, copyOwn(defaults, toVars), mergedTiming);
        }

        var api = {
            set: function(targets, vars, pos) {
                var start = parsePosition(pos != null ? pos : vars && vars.at, state);
                if (ctx.currentFrame >= baseDelay + start) {
                    ctx.set(targets, vars || {});
                }
                state.cursor = Math.max(state.cursor, start);
                recordChild(start, 0);
                return api;
            },
            to: function(targets, vars, pos) {
                addTween('to', targets, vars, null, pos);
                return api;
            },
            from: function(targets, vars, pos) {
                addTween('from', targets, vars, null, pos);
                return api;
            },
            fromTo: function(targets, fromVars, toVars, pos) {
                addTween('fromTo', targets, fromVars, toVars, pos);
                return api;
            },
            addLabel: function(name, pos) {
                state.labels[String(name)] = parsePosition(pos, state);
                return api;
            },
        };
        return api;
    };
})();
