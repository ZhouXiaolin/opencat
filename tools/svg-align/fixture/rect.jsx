export const meta = { width: 100, height: 100, fps: 30, duration: 0.04 };

/* Harness probe: same solid red as fixture/rect.svg, drawn with existing
   engine elements only. diff must be exact — validates resvg invocation,
   jsx render path and the diff math itself, before any svg backend exists.
   Styling goes through the sidecar tailwind.css (standard CSS engine; the
   engine CSS subset wants background-color, not the background shorthand). */
export default function SvgAlignProbe() {
  return <div id="root" class="probe-red" />;
}
