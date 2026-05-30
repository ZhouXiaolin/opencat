(function() {
    var runtime = globalThis.__opencatAnimation;

    // CSS Filter color matrix implementations
    // Based on: https://www.w3.org/TR/filter-effects-1/

    function brightnessMatrix(value) {
        var v = value;
        return [
            v, 0, 0, 0, 0,
            0, v, 0, 0, 0,
            0, 0, v, 0, 0,
            0, 0, 0, 1, 0
        ];
    }

    function contrastMatrix(value) {
        var v = value;
        var intercept = 0.5 * (1 - v);
        return [
            v, 0, 0, 0, intercept,
            0, v, 0, 0, intercept,
            0, 0, v, 0, intercept,
            0, 0, 0, 1, 0
        ];
    }

    function grayscaleMatrix(value) {
        var a = value;
        var b = 1 - a;
        return [
            b + a * 0.2126, a * 0.7152, a * 0.0722, 0, 0,
            a * 0.2126, b + a * 0.7152, a * 0.0722, 0, 0,
            a * 0.2126, a * 0.7152, b + a * 0.0722, 0, 0,
            0, 0, 0, 1, 0
        ];
    }

    function hueRotateMatrix(degrees) {
        var radians = degrees * Math.PI / 180;
        var cos = Math.cos(radians);
        var sin = Math.sin(radians);

        return [
            0.2126 + cos * 0.7874 - sin * 0.2126,
            0.7152 - cos * 0.7152 - sin * 0.7152,
            0.0722 - cos * 0.0722 + sin * 0.9278, 0, 0,
            0.2126 - cos * 0.2126 + sin * 0.1437,
            0.7152 + cos * 0.2848 + sin * 0.1400,
            0.0722 - cos * 0.0722 - sin * 0.2837, 0, 0,
            0.2126 - cos * 0.2126 - sin * 0.7874,
            0.7152 - cos * 0.7152 + sin * 0.7152,
            0.0722 + cos * 0.9278 + sin * 0.0722, 0, 0,
            0, 0, 0, 1, 0
        ];
    }

    function invertMatrix(value) {
        var a = value;
        var b = 1 - 2 * a;
        return [
            b, 0, 0, 0, a,
            0, b, 0, 0, a,
            0, 0, b, 0, a,
            0, 0, 0, 1, 0
        ];
    }

    function saturateMatrix(value) {
        var v = value;
        var a = (1 - v) * 0.2126;
        var b = (1 - v) * 0.7152;
        var c = (1 - v) * 0.0722;

        return [
            a + v, b, c, 0, 0,
            a, b + v, c, 0, 0,
            a, b, c + v, 0, 0,
            0, 0, 0, 1, 0
        ];
    }

    function sepiaMatrix(value) {
        var a = value;
        var b = 1 - a;

        return [
            b + a * 0.393, a * 0.769, a * 0.189, 0, 0,
            a * 0.349, b + a * 0.686, a * 0.168, 0, 0,
            a * 0.272, a * 0.534, b + a * 0.131, 0, 0,
            0, 0, 0, 1, 0
        ];
    }

    // Multiply two 4x5 matrices: result = a * b
    // Note: order matters! a is applied first, then b
    function multiplyMatrices(a, b) {
        var result = new Array(20);
        for (var i = 0; i < 4; i++) {
            for (var j = 0; j < 5; j++) {
                var sum = 0;
                for (var k = 0; k < 4; k++) {
                    sum += a[i * 5 + k] * b[k * 5 + j];
                }
                if (j === 4) {
                    sum += a[i * 5 + 4];
                }
                result[i * 5 + j] = sum;
            }
        }
        return result;
    }

    function identityMatrix() {
        return [
            1, 0, 0, 0, 0,
            0, 1, 0, 0, 0,
            0, 0, 1, 0, 0,
            0, 0, 0, 1, 0
        ];
    }

    function normalizeFilterName(name) {
        if (name === 'hue-rotate' || name === 'hueRotate' || name === 'huerotate') return 'hueRotate';
        return String(name).replace(/-([a-z])/g, function(_, c) { return c.toUpperCase(); });
    }

    function cssFilterName(name) {
        return name === 'hueRotate' ? 'hue-rotate' : name;
    }

    function parseFilterNumber(raw, name) {
        var str = String(raw).trim();
        var value = parseFloat(str);
        if (isNaN(value)) return NaN;
        if (str.slice(-1) === '%' && name !== 'blur' && name !== 'hueRotate') {
            return value / 100;
        }
        return value;
    }

    function filterOpToString(op) {
        var name = normalizeFilterName(op.name || op.kind);
        var unit = getFilterUnit(name);
        return cssFilterName(name) + '(' + Number(op.value) + unit + ')';
    }

    function filterValueToString(value) {
        if (!value) return '';
        if (typeof value === 'string') return value;
        if (value && Array.isArray(value.ops)) {
            var parts = [];
            for (var i = 0; i < value.ops.length; i++) {
                parts.push(filterOpToString(value.ops[i]));
            }
            return parts.join(' ');
        }
        return String(value);
    }

    function applyFilterValue(target, name, value) {
        if (target.node) {
            target.node[name](value);
            return;
        }
        if (target.set) {
            var values = {};
            values[name] = value;
            target.set(values);
        }
    }

    // Get matrix builder for a filter name
    function getFilterMatrixBuilder(name) {
        name = normalizeFilterName(name);
        switch (name) {
            case 'brightness': return brightnessMatrix;
            case 'contrast': return contrastMatrix;
            case 'grayscale': return grayscaleMatrix;
            case 'hueRotate': return hueRotateMatrix;
            case 'invert': return invertMatrix;
            case 'saturate': return saturateMatrix;
            case 'sepia': return sepiaMatrix;
            default: return null;
        }
    }

    // Parse filter string and build combined color matrix
    // IMPORTANT: Filters are applied in the order they appear in the string
    function buildFilterMatrix(filterStr) {
        if (!filterStr || typeof filterStr !== 'string') {
            return identityMatrix();
        }

        var matrix = identityMatrix();
        var regex = /(\w[\w-]*)\(([^)]*)\)/g;
        var match;

        while ((match = regex.exec(filterStr)) !== null) {
            var name = normalizeFilterName(match[1]);
            var value = match[2].trim();
            var numValue = parseFilterNumber(value, name);

            if (isNaN(numValue)) continue;

            // Skip blur - it's handled separately
            if (name === 'blur') continue;

            var builder = getFilterMatrixBuilder(name);
            if (builder) {
                var filterMatrix = builder(numValue);
                // Apply this filter: new_matrix = filter * current_matrix
                // This means the filter is applied AFTER the current accumulated effect
                matrix = multiplyMatrices(filterMatrix, matrix);
            }
        }

        return matrix;
    }

    // Individual filter property descriptors
    var filterProperties = {
        blur: {
            aliases: ['blurSigma'],
            defaultValue: 0,
            interpolate: 'number',
            apply: function(target, value) {
                applyFilterValue(target, 'blur', value);
            }
        },
        brightness: {
            defaultValue: 1,
            interpolate: 'number',
            apply: function(target, value) {
                applyFilterValue(target, 'brightness', value);
            }
        },
        contrast: {
            defaultValue: 1,
            interpolate: 'number',
            apply: function(target, value) {
                applyFilterValue(target, 'contrast', value);
            }
        },
        grayscale: {
            defaultValue: 0,
            interpolate: 'number',
            apply: function(target, value) {
                applyFilterValue(target, 'grayscale', value);
            }
        },
        hueRotate: {
            defaultValue: 0,
            interpolate: 'number',
            apply: function(target, value) {
                applyFilterValue(target, 'hueRotate', value);
            }
        },
        invert: {
            defaultValue: 0,
            interpolate: 'number',
            apply: function(target, value) {
                applyFilterValue(target, 'invert', value);
            }
        },
        saturate: {
            defaultValue: 1,
            interpolate: 'number',
            apply: function(target, value) {
                applyFilterValue(target, 'saturate', value);
            }
        },
        sepia: {
            defaultValue: 0,
            interpolate: 'number',
            apply: function(target, value) {
                applyFilterValue(target, 'sepia', value);
            }
        }
    };

    // Register individual filter properties
    runtime.animation.registerPlugin({
        name: 'filter-properties',
        properties: filterProperties
    });

    // Special 'filter' property that parses CSS filter string
    // Filters are applied in order: the first filter in the string is applied first
    runtime.animation.registerProperty('filter', {
        defaultValue: '',
        // Use custom interpolation function for filter strings
        interpolate: function(fromVal, toVal, progress) {
            return interpolateFilterStrings(fromVal, toVal, progress);
        },
        apply: function(target, value) {
            if (!value) return;
            if (target.node) {
                target.node.filter(value);
                return;
            }
            if (target.set) target.set({ filter: value });
        }
    });

    function interpolateFilterStrings(from, to, progress) {
        var fromOps = parseFilterOps(from);
        var toOps = parseFilterOps(to);

        if (toOps.length === 0 && fromOps.length === 0) return 'none';

        var sameShape = fromOps.length === toOps.length;
        for (var i = 0; i < fromOps.length && sameShape; i++) {
            sameShape = fromOps[i].name === toOps[i].name;
        }

        var output = [];
        if (sameShape) {
            for (var i = 0; i < toOps.length; i++) {
                output.push({
                    name: toOps[i].name,
                    value: fromOps[i].value + (toOps[i].value - fromOps[i].value) * progress
                });
            }
        } else {
            var usedFrom = {};
            for (var i = 0; i < toOps.length; i++) {
                var toOp = toOps[i];
                var fromIndex = findNextFilterOp(fromOps, toOp.name, usedFrom);
                var fromValue = fromIndex >= 0 ? fromOps[fromIndex].value : getDefaultFilterValue(toOp.name);
                if (fromIndex >= 0) usedFrom[fromIndex] = true;
                output.push({
                    name: toOp.name,
                    value: fromValue + (toOp.value - fromValue) * progress
                });
            }
            for (var i = 0; i < fromOps.length; i++) {
                if (usedFrom[i]) continue;
                output.push({
                    name: fromOps[i].name,
                    value: fromOps[i].value + (getDefaultFilterValue(fromOps[i].name) - fromOps[i].value) * progress
                });
            }
        }

        var parts = [];
        for (var i = 0; i < output.length; i++) {
            parts.push(filterOpToString(output[i]));
        }
        return parts.join(' ') || 'none';
    }

    function findNextFilterOp(ops, name, used) {
        for (var i = 0; i < ops.length; i++) {
            if (!used[i] && ops[i].name === name) return i;
        }
        return -1;
    }

    function parseFilterOps(value) {
        var result = [];
        if (value && Array.isArray(value.ops)) {
            for (var i = 0; i < value.ops.length; i++) {
                var op = value.ops[i];
                var name = normalizeFilterName(op.kind || op.name);
                var numValue = Number(op.value);
                if (!isNaN(numValue)) result.push({ name: name, value: numValue });
            }
            return result;
        }

        var str = filterValueToString(value);
        if (!str || typeof str !== 'string') return result;

        var regex = /(\w[\w-]*)\(([^)]*)\)/g;
        var match;

        while ((match = regex.exec(str)) !== null) {
            var name = normalizeFilterName(match[1]);
            var numValue = parseFilterNumber(match[2], name);

            if (!isNaN(numValue)) {
                result.push({ name: name, value: numValue });
            }
        }

        return result;
    }

    function getDefaultFilterValue(name) {
        switch (name) {
            case 'brightness': return 1;
            case 'contrast': return 1;
            case 'grayscale': return 0;
            case 'hueRotate': return 0;
            case 'invert': return 0;
            case 'saturate': return 1;
            case 'sepia': return 0;
            case 'blur': return 0;
            default: return 0;
        }
    }

    function getFilterUnit(name) {
        switch (name) {
            case 'blur': return 'px';
            case 'hueRotate': return 'deg';
            default: return '';
        }
    }

    // Export matrix functions for renderer use
    runtime.filterMatrix = {
        brightness: brightnessMatrix,
        contrast: contrastMatrix,
        grayscale: grayscaleMatrix,
        hueRotate: hueRotateMatrix,
        invert: invertMatrix,
        saturate: saturateMatrix,
        sepia: sepiaMatrix,
        identity: identityMatrix,
        multiply: multiplyMatrices,
        buildFromFilterString: buildFilterMatrix
    };
})();
