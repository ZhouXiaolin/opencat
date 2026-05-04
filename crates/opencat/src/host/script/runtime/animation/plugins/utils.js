(function() {
    var runtime = globalThis.__opencatAnimation;

    runtime.animation.registerPlugin({
        name: 'utils',
        install: function() {
            ctx.utils = {
                clamp: function(value, min, max) {
                    return Math.min(max, Math.max(min, Number(value)));
                },
                snap: function(value, step) {
                    var n = Number(value);
                    var s = Number(step);
                    if (!isFinite(s) || s <= 0) return n;
                    return Math.round(n / s) * s;
                },
                wrap: function(value, min, max) {
                    var lo = Number(min);
                    var hi = Number(max);
                    var range = hi - lo;
                    if (range <= 0) return lo;
                    var n = (Number(value) - lo) % range;
                    if (n < 0) n += range;
                    return n + lo;
                },
                mapRange: function(value, inMin, inMax, outMin, outMax) {
                    var iMin = Number(inMin);
                    var iMax = Number(inMax);
                    if (iMax === iMin) return Number(outMin);
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
        },
    });
})();
