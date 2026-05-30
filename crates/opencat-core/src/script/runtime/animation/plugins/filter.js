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

    // Get matrix builder for a filter name
    function getFilterMatrixBuilder(name) {
        switch (name) {
            case 'brightness': return brightnessMatrix;
            case 'contrast': return contrastMatrix;
            case 'grayscale': return grayscaleMatrix;
            case 'hue-rotate': case 'hueRotate': return hueRotateMatrix;
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
            var name = match[1].toLowerCase();
            var value = match[2].trim();
            var numValue = parseFloat(value);

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
        brightness: {
            defaultValue: 1,
            interpolate: 'number',
            apply: function(target, value) {
                if (target.set) {
                    target.set({ brightness: value });
                }
            }
        },
        contrast: {
            defaultValue: 1,
            interpolate: 'number',
            apply: function(target, value) {
                if (target.set) {
                    target.set({ contrast: value });
                }
            }
        },
        grayscale: {
            defaultValue: 0,
            interpolate: 'number',
            apply: function(target, value) {
                if (target.set) {
                    target.set({ grayscale: value });
                }
            }
        },
        hueRotate: {
            defaultValue: 0,
            interpolate: 'number',
            apply: function(target, value) {
                if (target.set) {
                    target.set({ hueRotate: value });
                }
            }
        },
        invert: {
            defaultValue: 0,
            interpolate: 'number',
            apply: function(target, value) {
                if (target.set) {
                    target.set({ invert: value });
                }
            }
        },
        saturate: {
            defaultValue: 1,
            interpolate: 'number',
            apply: function(target, value) {
                if (target.set) {
                    target.set({ saturate: value });
                }
            }
        },
        sepia: {
            defaultValue: 0,
            interpolate: 'number',
            apply: function(target, value) {
                if (target.set) {
                    target.set({ sepia: value });
                }
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
            if (!value || typeof value !== 'string') return;
            
            // Parse filters in order and build result
            var regex = /(\w[\w-]*)\(([^)]*)\)/g;
            var match;
            var result = {};

            while ((match = regex.exec(value)) !== null) {
                var name = match[1].toLowerCase();
                var numValue = parseFloat(match[2]);

                if (isNaN(numValue)) continue;

                // Convert to camelCase
                var camelName = name.replace(/-([a-z])/g, function(_, c) { return c.toUpperCase(); });
                
                result[camelName] = numValue;
            }

            if (target.set) {
                target.set(result);
            }
        }
    });

    function interpolateFilterStrings(from, to, progress) {
        // Parse both filter strings preserving order
        var fromParsed = parseFilterString(from);
        var toParsed = parseFilterString(to);
        
        // Merge all filter names preserving order from 'to'
        var allFilters = {};
        var order = [];
        
        // Add 'from' filters
        for (var i = 0; i < fromParsed.order.length; i++) {
            var name = fromParsed.order[i];
            allFilters[name] = { from: fromParsed.values[name], to: getDefaultFilterValue(name) };
            order.push(name);
        }
        
        // Add 'to' filters (may override 'from' values)
        for (var i = 0; i < toParsed.order.length; i++) {
            var name = toParsed.order[i];
            if (allFilters[name]) {
                allFilters[name].to = toParsed.values[name];
            } else {
                allFilters[name] = { from: getDefaultFilterValue(name), to: toParsed.values[name] };
                order.push(name);
            }
        }

        // Interpolate and build result string in order
        var parts = [];
        var blurPart = null;
        
        for (var i = 0; i < order.length; i++) {
            var name = order[i];
            var filter = allFilters[name];
            var interpolated = filter.from + (filter.to - filter.from) * progress;
            
            var unit = getFilterUnit(name);
            var part = name + '(' + interpolated + unit + ')';
            
            if (name === 'blur') {
                blurPart = part;
            } else {
                parts.push(part);
            }
        }

        // Blur goes first if present
        if (blurPart) {
            parts.unshift(blurPart);
        }

        return parts.join(' ') || 'none';
    }

    function parseFilterString(str) {
        var result = { values: {}, order: [] };
        if (!str || typeof str !== 'string') return result;
        
        var regex = /(\w[\w-]*)\(([^)]*)\)/g;
        var match;
        
        while ((match = regex.exec(str)) !== null) {
            var name = match[1].toLowerCase();
            var numValue = parseFloat(match[2]);
            
            if (!isNaN(numValue)) {
                var camelName = name.replace(/-([a-z])/g, function(_, c) { return c.toUpperCase(); });
                result.values[camelName] = numValue;
                result.order.push(camelName);
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
