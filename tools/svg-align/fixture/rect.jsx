export const meta = { width: 100, height: 100, fps: 30, duration: 0.04 };

/* Harness probe: fixture/rect.svg through the <svg src> backend, diffed
   against the resvg render of the same file. diff must be exact — validates
   resvg invocation, the svg backend pipeline and the diff math itself.
   Positioning goes through the sidecar tailwind.css (standard CSS engine). */
export default function SvgAlignProbe() {
  return (
    <div id="root" class="probe-box">
      <svg src="F:/OpenCat/tools/svg-align/fixture/rect.svg" width="100" height="100" class="probe-svg" />
    </div>
  );
}
