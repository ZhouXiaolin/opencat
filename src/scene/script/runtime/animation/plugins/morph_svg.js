(function() {
    var runtime = globalThis.__opencatAnimation;

    runtime.animation.registerPlugin({
        name: 'morph-svg',
        properties: {
            svgPath: {
                defaultValue: '',
                aliases: ['d'],
                prepare: function(ctx) {
                    var fromStr = String(ctx.from);
                    var toStr = String(ctx.to);
                    var grid = ctx.timing.gridResolution || 128;
                    var handle = __morph_svg_create(fromStr, toStr, Number(grid));
                    return { handle: handle };
                },
                sample: function(ctx) {
                    if (ctx.prepared && ctx.prepared.handle != null) {
                        var tol = ctx.timing.simplifyTolerance || 0;
                        return __morph_svg_sample(ctx.prepared.handle, ctx.progress, Number(tol));
                    }
                    if (ctx.progress >= 1) return String(ctx.to);
                    return String(ctx.from);
                },
                apply: function(target, value) {
                    if (target.node) {
                        target.node.svgPath(value);
                    }
                },
            },
        },
    });
})();
