(function() {
    function applyMutation(id, prop, ...args) {
        __write_style_value(id, prop, args.length > 1 ? args : args[0]);
    }

    function assertVisualTarget(id, apiName) {
        var key = String(id);
        var registry = ctx.__targetRegistry || {};
        if (registry.visual && registry.visual[key]) return key;
        if (registry.nonVisual && registry.nonVisual[key]) {
            throw new Error(apiName + ": non-visual id '" + key + "' cannot be targeted");
        }
        throw new Error(apiName + ": unknown id '" + key + "'");
    }

    const nodeCache = {};
    ctx.getNode = function(id) {
        id = assertVisualTarget(id, 'ctx.getNode');
        if (!nodeCache[id]) {
            let api = null;
            api = new Proxy({}, {
                get(target, prop) {
                    if (typeof prop !== 'string' || prop === 'then') {
                        return undefined;
                    }
                    return (...args) => {
                        applyMutation(id, prop, ...args);
                        return api;
                    };
                }
            });
            nodeCache[id] = api;
        }
        return nodeCache[id];
    };
})();
