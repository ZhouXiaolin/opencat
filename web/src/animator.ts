// ── Animation Runtime (mirrors Rust QuickJS animation system) ──
// Handles: __animate_create, __animate_value, __animate_color, __animate_progress
// Easing functions and spring physics

// ── Easing Functions ──

type EasingFn = (t: number) => number;

const easeTable: Record<string, EasingFn> = {
  'linear': (t) => t,
  'ease': (t) => smoothstep(t),
  'ease-in': (t) => t * t,
  'ease-out': (t) => 1 - (1 - t) * (1 - t),
  'ease-in-out': (t) => t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2,
  'sine.in': (t) => 1 - Math.cos((t * Math.PI) / 2),
  'sine.out': (t) => Math.sin((t * Math.PI) / 2),
  'sine.inOut': (t) => -(Math.cos(Math.PI * t) - 1) / 2,
  'quad.in': (t) => t * t,
  'quad.out': (t) => 1 - (1 - t) * (1 - t),
  'quad.inOut': (t) => t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2,
  'cubic.in': (t) => t * t * t,
  'cubic.out': (t) => 1 - Math.pow(1 - t, 3),
  'cubic.inOut': (t) => t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2,
  'quart.in': (t) => t * t * t * t,
  'quart.out': (t) => 1 - Math.pow(1 - t, 4),
  'quart.inOut': (t) => t < 0.5 ? 8 * t * t * t * t : 1 - Math.pow(-2 * t + 2, 4) / 2,
  'expo.in': (t) => t === 0 ? 0 : Math.pow(2, 10 * t - 10),
  'expo.out': (t) => t === 1 ? 1 : 1 - Math.pow(2, -10 * t),
  'expo.inOut': (t) => {
    if (t === 0) return 0;
    if (t === 1) return 1;
    return t < 0.5
      ? Math.pow(2, 20 * t - 10) / 2
      : (2 - Math.pow(2, -20 * t + 10)) / 2;
  },
  'circ.in': (t) => 1 - Math.sqrt(1 - t * t),
  'circ.out': (t) => Math.sqrt(1 - Math.pow(t - 1, 2)),
  'circ.inOut': (t) => {
    return t < 0.5
      ? (1 - Math.sqrt(1 - Math.pow(2 * t, 2))) / 2
      : (Math.sqrt(1 - Math.pow(-2 * t + 2, 2)) + 1) / 2;
  },
  'back.in': (t) => (2.70158 * t - 1.70158) * t * t,
  'back.out': (t) => 1 + 2.70158 * Math.pow(t - 1, 3) + 1.70158 * Math.pow(t - 1, 2),
  'back.inOut': (t) => {
    const c1 = 1.70158;
    const c2 = c1 * 1.525;
    return t < 0.5
      ? (Math.pow(2 * t, 2) * ((c2 + 1) * 2 * t - c2)) / 2
      : (Math.pow(2 * t - 2, 2) * ((c2 + 1) * (t * 2 - 2) + c2) + 2) / 2;
  },
  'elastic.in': (t) => {
    if (t === 0 || t === 1) return t;
    const c4 = (2 * Math.PI) / 3;
    return -Math.pow(2, 10 * t - 10) * Math.sin((t * 10 - 10.75) * c4);
  },
  'elastic.out': (t) => {
    if (t === 0 || t === 1) return t;
    const c4 = (2 * Math.PI) / 3;
    return Math.pow(2, -10 * t) * Math.sin((t * 10 - 0.75) * c4) + 1;
  },
  'bounce.in': (t) => 1 - bounceOut(1 - t),
  'bounce.out': (t) => bounceOut(t),
  'bounce.inOut': (t) => t < 0.5
    ? (1 - bounceOut(1 - 2 * t)) / 2
    : (1 + bounceOut(2 * t - 1)) / 2,
};

function smoothstep(t: number): number {
  return t * t * (3 - 2 * t);
}

function bounceOut(t: number): number {
  const n1 = 7.5625;
  const d1 = 2.75;
  if (t < 1 / d1) {
    return n1 * t * t;
  } else if (t < 2 / d1) {
    return n1 * (t -= 1.5 / d1) * t + 0.75;
  } else if (t < 2.5 / d1) {
    return n1 * (t -= 2.25 / d1) * t + 0.9375;
  } else {
    return n1 * (t -= 2.625 / d1) * t + 0.984375;
  }
}

export function getEasing(name: string): EasingFn {
  return easeTable[name] || easeTable['ease'];
}

// ── Spring Physics ──

export interface SpringConfig {
  mass: number;
  stiffness: number;
  damping: number;
}

export function springValue(
  t: number,
  from: number,
  to: number,
  config: SpringConfig,
): number {
  const { mass, stiffness, damping } = config;
  const delta = to - from;

  // Critically damped spring
  const omega0 = Math.sqrt(stiffness / mass);
  const zeta = damping / (2 * Math.sqrt(mass * stiffness));

  if (zeta < 1) {
    // Underdamped
    const omega1 = omega0 * Math.sqrt(1 - zeta * zeta);
    const envelope = Math.exp(-zeta * omega0 * t);
    return from + delta * (1 - envelope * (Math.cos(omega1 * t) + (zeta / Math.sqrt(1 - zeta * zeta)) * Math.sin(omega1 * t)));
  } else {
    // Critically damped
    const envelope = Math.exp(-omega0 * t);
    return from + delta * (1 - envelope * (1 + omega0 * t));
  }
}

// ── Animation Track ──

export interface AnimEntry {
  duration: number;
  delay: number;
  clamp: boolean;
  easing: string | null;
  springConfig: SpringConfig | null;
  repeat: number;
  yoyo: boolean;
  repeatDelay: number;
}

// ── Easing name parsing (matches Rust easing_from_name) ──

export function parseEasing(tag: string): { easing: string | null; spring: SpringConfig | null } {
  if (tag.startsWith('spring.')) {
    const variant = tag.slice(7);
    switch (variant) {
      case 'gentle': return { easing: null, spring: { mass: 1, stiffness: 100, damping: 15 } };
      case 'wobbly': return { easing: null, spring: { mass: 1, stiffness: 180, damping: 12 } };
      case 'stiff': return { easing: null, spring: { mass: 1, stiffness: 300, damping: 20 } };
      case 'slow': return { easing: null, spring: { mass: 3, stiffness: 100, damping: 20 } };
      case 'molasses': return { easing: null, spring: { mass: 6, stiffness: 60, damping: 18 } };
      default: return { easing: null, spring: { mass: 1, stiffness: 100, damping: 15 } };
    }
  }
  return { easing: tag, spring: null };
}

// ── Compute animation progress (mirrors Rust's compute_progress) ──

export function computeProgress(
  currentFrame: number,
  duration: number,
  delay: number,
  easing: string | null,
  springConfig: SpringConfig | null,
  clamp: boolean,
  repeat: number,
  yoyo: boolean,
  repeatDelay: number,
): number {
  if (currentFrame < delay) return clamp ? 0 : -1;

  const localFrame = currentFrame - delay;
  const totalFrames = repeat >= 0
    ? duration + (repeat > 0 ? repeat * (duration + repeatDelay) : 0)
    : Number.MAX_SAFE_INTEGER;

  if (localFrame >= totalFrames) return 1;

  let effectiveFrame: number;
  let cycleIndex: number;

  if (repeat < 0) {
    effectiveFrame = localFrame % (duration + repeatDelay);
    cycleIndex = Math.floor(localFrame / (duration + repeatDelay));
  } else {
    const cycleLen = duration + repeatDelay;
    if (localFrame >= totalFrames) return 1;
    effectiveFrame = localFrame % cycleLen;
    cycleIndex = Math.floor(localFrame / cycleLen);
  }

  let rawProgress: number;
  if (effectiveFrame >= duration) {
    rawProgress = 1;
  } else {
    const t = effectiveFrame / duration;
    rawProgress = applyEasing(t, easing, springConfig);
  }

  if (yoyo && cycleIndex % 2 === 1) {
    rawProgress = 1 - rawProgress;
  }

  return rawProgress;
}

function applyEasing(t: number, easing: string | null, springConfig: SpringConfig | null): number {
  if (springConfig) {
    return springValue(t, 0, 1, springConfig);
  }
  if (easing) {
    return getEasing(easing)(t);
  }
  return t;
}

// ── Animate Value (matches Rust's animate_value) ──

export function animateValue(
  currentFrame: number,
  duration: number,
  delay: number,
  from: number,
  to: number,
  easing: string | null,
  springConfig: SpringConfig | null,
  clamp: boolean,
  repeat: number,
  yoyo: boolean,
  repeatDelay: number,
): number {
  const progress = computeProgress(
    currentFrame, duration, delay,
    easing, springConfig, clamp,
    repeat, yoyo, repeatDelay,
  );
  return from + (to - from) * progress;
}

// ── Color lerp (matches Rust's lerp_hsla) ──

export interface HSLA { h: number; s: number; l: number; a: number }

export function parseColorToHsla(color: string): HSLA | null {
  // Parse hex colors
  const hexMatch = color.match(/^#?([0-9a-fA-F]{2})([0-9a-fA-F]{2})([0-9a-fA-F]{2})([0-9a-fA-F]{2})?$/);
  if (hexMatch) {
    const r = parseInt(hexMatch[1], 16) / 255;
    const g = parseInt(hexMatch[2], 16) / 255;
    const b = parseInt(hexMatch[3], 16) / 255;
    const a = hexMatch[4] ? parseInt(hexMatch[4], 16) / 255 : 1;
    return rgbToHsla(r, g, b, a);
  }

  // Parse short hex
  const shortMatch = color.match(/^#?([0-9a-fA-F])([0-9a-fA-F])([0-9a-fA-F])$/);
  if (shortMatch) {
    const r = parseInt(shortMatch[1] + shortMatch[1], 16) / 255;
    const g = parseInt(shortMatch[2] + shortMatch[2], 16) / 255;
    const b = parseInt(shortMatch[3] + shortMatch[3], 16) / 255;
    return rgbToHsla(r, g, b, 1);
  }

  return null;
}

export function hslaToString(hsla: HSLA): string {
  return `rgba(${Math.round(hsla.h)}, ${Math.round(hsla.s)}, ${Math.round(hsla.l)}, ${hsla.a.toFixed(3)})`;
}

function rgbToHsla(r: number, g: number, b: number, a: number): HSLA {
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  let h = 0, s = 0;
  const l = (max + min) / 2;

  if (max !== min) {
    const d = max - min;
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    switch (max) {
      case r: h = (g - b) / d + (g < b ? 6 : 0); break;
      case g: h = (b - r) / d + 2; break;
      case b: h = (r - g) / d + 4; break;
    }
    h /= 6;
  }

  return { h: h * 360, s: s * 100, l: l * 100, a };
}

export function lerpHsla(from: HSLA, to: HSLA, t: number): HSLA {
  // Shortest path hue interpolation
  let dh = to.h - from.h;
  if (dh > 180) dh -= 360;
  if (dh < -180) dh += 360;

  return {
    h: from.h + dh * t,
    s: from.s + (to.s - from.s) * t,
    l: from.l + (to.l - from.l) * t,
    a: from.a + (to.a - from.a) * t,
  };
}

export function animateColor(
  currentFrame: number,
  duration: number,
  delay: number,
  from: string,
  to: string,
  easing: string | null,
  springConfig: SpringConfig | null,
  clamp: boolean,
  repeat: number,
  yoyo: boolean,
  repeatDelay: number,
): string {
  const fromHSL = parseColorToHsla(from);
  const toHSL = parseColorToHsla(to);
  if (!fromHSL || !toHSL) return from;

  const progress = computeProgress(
    currentFrame, duration, delay,
    easing, springConfig, clamp,
    repeat, yoyo, repeatDelay,
  );

  const result = lerpHsla(fromHSL, toHSL, progress);
  return hslaToString(result);
}
