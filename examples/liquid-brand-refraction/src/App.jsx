import { Composition, COMPOSITION_DURATION, onFrame } from './runtime';
import { initLiquid, renderAt } from './legacy/liquid-pop-v2';

const hidden = { position: 'absolute', width: 1, height: 1, opacity: 0 };

export default function App() {
  var started = false;
  onFrame(function (T) {
    if (!started) { started = true; initLiquid(); }
    renderAt(T);
  });
  return (
    <Composition id="main" duration={COMPOSITION_DURATION} width={1920} height={1080} fps={30}
      style={{ position: 'relative', width: 1920, height: 1080, background: '#f4f2ea', overflow: 'hidden' }}>
      <video id="facet-video-0" style={hidden} data-start="0" data-duration="6" src="assets/01.mp4" muted playsinline />
      <video id="facet-video-1" style={hidden} data-start="0" data-duration="6" src="assets/02.mp4" muted playsinline />
      <video id="facet-video-2" style={hidden} data-start="0" data-duration="6" src="assets/03.mp4" muted playsinline />
      <video id="facet-video-3" style={hidden} data-start="0" data-duration="6" src="assets/04.mp4" muted playsinline />
      <video id="facet-video-4" style={hidden} data-start="0" data-duration="6" src="assets/05.mp4" muted playsinline />
      <video id="facet-video-5" style={hidden} data-start="0" data-duration="6" src="assets/06.mp4" muted playsinline />
      <video id="facet-video-6" style={hidden} data-start="0" data-duration="6" src="assets/07.mp4" muted playsinline />
      <video id="facet-video-7" style={hidden} data-start="0" data-duration="6" src="assets/08.mp4" muted playsinline />
      <canvas id="liquid-pop-canvas" data-start="0" data-duration="6"
        style={{ position: 'absolute', inset: 0, width: 1920, height: 1080 }} />
    </Composition>
  );
}
