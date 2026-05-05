(function() {
    var runtime = globalThis.__opencatAnimation;

    runtime.animation.registerPlugin({
        name: 'motion-path',
        specialOptions: ['path', 'orient'],

        augmentTween: function(tween) {
            if (!(tween.timing.path || tween.toVars.path)) {
                return;
            }
            var svg = String(tween.timing.path || tween.toVars.path);
            var orient = tween.timing.orient !== undefined
                ? Number(tween.timing.orient)
                : Number(tween.toVars.orient || 0);
            var cacheKey = '__ap_' + svg;
            if (!ctx[cacheKey]) {
                ctx[cacheKey] = __along_path_create(svg);
            }
            var handle = ctx[cacheKey];

            tween.addTrack('x');
            tween.addTrack('y');
            tween.addTrack('rotation');

            function samplePath() {
                return __along_path_at(handle, __animate_progress(tween.handle));
            }

            tween.sampleOverrides.x = function() {
                return samplePath()[0];
            };
            tween.sampleOverrides.y = function() {
                return samplePath()[1];
            };
            tween.sampleOverrides.rotation = function() {
                return samplePath()[2] + orient;
            };
        },
    });
})();
