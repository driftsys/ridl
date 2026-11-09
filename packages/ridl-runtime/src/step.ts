// The run-time half of the float `step` check of a generated `make<Name>`
// constructor. The compiler prepares the lattice constants (`prepare_float_step`
// in the ridl-ir crate, which the Rust backend uses too); the functions here
// only evaluate them, with primitive operations.

/** The prepared lattice: factor, original step, step high and low, phase high and low. */
export type FloatLattice = readonly [
  number,
  number,
  number,
  number,
  number,
  number,
];

const view = new DataView(new ArrayBuffer(8));

// The unit in the last place of the 32-bit float format is 2^-23.
const EPSILON_32 = 2 ** -23;
const MAX = Number.MAX_VALUE;

function abs(value: number): number {
  return Math.abs(value);
}

/** The value rounded to the wire precision. */
function project(value: number, wire32: boolean): number {
  return wire32 ? Math.fround(value) : value;
}

/**
 * The floats next to `magnitude` (a non-negative value at the wire precision),
 * as `[below, above]`. Zero has the negative smallest subnormal below it.
 */
function neighbours(magnitude: number, wire32: boolean): [number, number] {
  if (wire32) {
    view.setFloat32(0, magnitude);
    const bits = view.getUint32(0);
    view.setUint32(0, bits === 0 ? 0x80000001 : bits - 1);
    const below = view.getFloat32(0);
    view.setUint32(0, bits + 1);
    return [below, view.getFloat32(0)];
  }
  view.setFloat64(0, magnitude);
  const bits = view.getBigUint64(0);
  view.setBigUint64(0, bits === 0n ? 0x8000000000000001n : bits - 1n);
  const below = view.getFloat64(0);
  view.setBigUint64(0, bits + 1n);
  return [below, view.getFloat64(0)];
}

/** The value with the low 27 bits of its mantissa cleared. */
function high(value: number): number {
  view.setFloat64(0, value);
  view.setUint32(4, (view.getUint32(4) & 0xf8000000) >>> 0);
  return view.getFloat64(0);
}

/** The product of two floats and its rounding error (Dekker's two-product). */
function productWithError(left: number, right: number): [number, number] {
  const product = left * right;
  const leftHigh = high(left);
  const rightHigh = high(right);
  const leftLow = left - leftHigh;
  const rightLow = right - rightHigh;
  const error = ((leftHigh * rightHigh - product) + leftHigh * rightLow +
    leftLow * rightHigh) + leftLow * rightLow;
  return [product, error];
}

/**
 * True when `value` is not on the lattice `phase + k * step` of a float with
 * the given wire precision. A non-finite value is never on the lattice.
 */
export function offFloatStep(
  value: number,
  wire32: boolean,
  lattice: FloatLattice,
): boolean {
  const [factor, originalStep, step, stepLow, phase, phaseLow] = lattice;
  const checked = project(value, wire32);
  if (!Number.isFinite(checked)) {
    return true;
  }
  const epsilon = wire32 ? EPSILON_32 : Number.EPSILON;
  // The adjacent floats define the rounding cell of the value.
  const [below, above] = neighbours(abs(checked), wire32);
  const represented = abs(checked);
  const lowerGap = represented - below;
  const upperGap = Number.isFinite(above) ? above - represented : lowerGap;
  const cell = (lowerGap + upperGap) / 2;
  if (originalStep < cell) {
    return false;
  }
  const scaledValue = checked * factor;
  const delta = scaledValue - phase;
  const quotient = Number.isFinite(delta)
    ? delta / step
    : scaledValue / step - phase / step;
  // Ties round away from zero.
  const fraction = quotient % 1;
  const index = quotient - fraction +
    (fraction >= 0.5 ? 1 : fraction <= -0.5 ? -1 : 0);
  const scale = Math.max(abs(checked), abs(phase / factor), abs(originalStep));
  const tolerance = Math.min(4 * epsilon * scale, originalStep / 4);
  // Scale large terms before reconstructing: a rounded high sum can overflow
  // before its negative decimal residual brings the point back into range.
  const rescale = Math.max(abs(scaledValue), abs(phase), abs(step)) > MAX / 4
    ? 0.25
    : 1;
  let distance = Infinity;
  // The estimate can select a neighbouring index through division rounding.
  for (const candidate of [index - 1, index, index + 1]) {
    const [product, productError] = productWithError(candidate, step * rescale);
    const scaledPhase = phase * rescale;
    const sum = product + scaledPhase;
    const part = sum - product;
    const sumError = (product - (sum - part)) + (scaledPhase - part);
    const low = candidate * (stepLow * rescale) + phaseLow * rescale +
      productError + sumError;
    const nearest = project((sum + low) / factor / rescale, wire32);
    if (Number.isFinite(nearest)) {
      distance = Math.min(distance, abs(checked - nearest));
    }
  }
  return !Number.isFinite(quotient) || distance > tolerance;
}

/**
 * True when `value` is not within a relative tolerance of one of `points`, the
 * finite lattice points that the compiler enumerated for a step too large for
 * a binary64. An empty list refuses every value.
 */
export function offFloatPoints(
  value: number,
  wire32: boolean,
  points: readonly number[],
): boolean {
  const checked = project(value, wire32);
  const epsilon = wire32 ? EPSILON_32 : Number.EPSILON;
  return !Number.isFinite(checked) ||
    !points.some((point) =>
      abs(checked - point) <= 4 * epsilon * Math.max(abs(checked), abs(point))
    );
}
