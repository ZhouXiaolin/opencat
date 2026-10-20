import { gsap } from 'gsap';
import { useGSAP } from 'opencat/runtime';
import { Tick, vis } from './shared.jsx';

export default function OpenCard() {
  useGSAP((ctx, t) => {
    var T = t;
    // Type-in two words, un-type via remnants, braces widen, fade.
    // Declarative tweens (absolute times); the `opencard` visibility window
    // stays a per-frame gate.
    var OPEN_ON = (T >= 0 && T < 1.12);
    vis(ctx, 'opencard', OPEN_ON);
    if (OPEN_ON) {
      // un-type: the braces snap inward to the remnant headline widths.
      // Offsets are the reference's computed values relative to the type-in
      // rest ink lefts (511 / 1371):
      //   bGapMid = midW/2 + 68  -> obl +67.5225, obr -66.5225
      //   bGapRem = remW/2 + 52  -> obl +325.5715, obr -324.5715
      var __tl = gsap.timeline()
        .set(['openmid', 'openrem', 'ot1', 'ot2'], { opacity: 0 }, 0)
        .set('ot1', { opacity: 1 }, 0.0333)
        .set('ot2', { opacity: 1 }, 0.0663)
        .set('opentext', { opacity: 0 }, 0.9333)
        .set('openmid', { opacity: 1 }, 0.9333)
        .set('obl', { x: 67.5225 }, 0.9333)
        .set('obr', { x: -66.5225 }, 0.9333)
        .set('openmid', { opacity: 0 }, 0.9663)
        .set('openrem', { opacity: 0.75 }, 0.9663)
        .set('obl', { x: 325.5715 }, 0.9663)
        .set('obr', { x: -324.5715 }, 0.9663)
        // fade out 1.0 → 1.07 (braces drift a further +24 / -24).
        // The reference omits `ease` here, so GSAP's default power1.out
        // applies; our engine default is linear, so it is spelled out.
        .to(['obl', 'obr', 'openrem'], { opacity: 0, duration: 0.07, ease: 'power1.in' }, 1.0)
        .to('obl', { x: 325.5715 + 24, duration: 0.07, ease: 'power1.out' }, 1.0)
        .to('obr', { x: -324.5715 - 24, duration: 0.07, ease: 'power1.out' }, 1.0)
        .to(['stripOL', 'stripOR'], { opacity: 0, duration: 0.12, ease: 'power1.out' }, 0.95);
    }
  });
  return (
    <div id="opencard" class="absolute inset-0 z-[9] opacity-0">
      <div id="stripOL" class="absolute left-[32px] top-[528px] flex items-end gap-[6.8px] opacity-90">
        <Tick id="d-566" ht="23px" /><Tick id="d-567" ht="10px" mb="mb-[6px]" />
        <Tick id="d-568" ht="23px" /><Tick id="d-569" ht="10px" mb="mb-[6px]" />
        <Tick id="d-570" ht="23px" /><Tick id="d-571" ht="10px" mb="mb-[6px]" />
        <Tick id="d-572" ht="23px" /><Tick id="d-573" ht="10px" mb="mb-[6px]" />
        <Tick id="d-574" ht="23px" /><Tick id="d-575" ht="10px" mb="mb-[6px]" />
        <Tick id="d-576" ht="23px" /><Tick id="d-577" ht="10px" mb="mb-[6px]" />
        <Tick id="d-578" ht="23px" /><Tick id="d-579" ht="10px" mb="mb-[6px]" />
        <Tick id="d-580" ht="23px" /><Tick id="d-581" ht="10px" mb="mb-[6px]" />
        <Tick id="d-582" ht="23px" /><Tick id="d-583" ht="10px" mb="mb-[6px]" />
        <Tick id="d-584" ht="23px" /><Tick id="d-585" ht="10px" mb="mb-[6px]" />
        <Tick id="d-586" ht="23px" /><Tick id="d-587" ht="10px" mb="mb-[6px]" />
        <Tick id="d-588" ht="23px" /><Tick id="d-589" ht="10px" mb="mb-[6px]" />
        <Tick id="d-590" ht="23px" /><Tick id="d-591" ht="10px" mb="mb-[6px]" />
        <Tick id="d-592" ht="23px" /><Tick id="d-593" ht="10px" mb="mb-[6px]" />
        <Tick id="d-594" ht="23px" /><Tick id="d-595" ht="10px" mb="mb-[6px]" />
        <Tick id="d-596" ht="23px" /><Tick id="d-597" ht="10px" mb="mb-[6px]" />
        <Tick id="d-598" ht="23px" /><Tick id="d-599" ht="10px" mb="mb-[6px]" />
        <Tick id="d-600" ht="23px" /><Tick id="d-601" ht="10px" mb="mb-[6px]" />
        <Tick id="d-602" ht="23px" />
      </div>
      <div id="stripOR" class="absolute left-[1627px] top-[528px] flex items-end gap-[6.8px] opacity-90">
        <Tick id="d-603" ht="23px" /><Tick id="d-604" ht="10px" mb="mb-[6px]" />
        <Tick id="d-605" ht="23px" /><Tick id="d-606" ht="10px" mb="mb-[6px]" />
        <Tick id="d-607" ht="23px" /><Tick id="d-608" ht="10px" mb="mb-[6px]" />
        <Tick id="d-609" ht="23px" /><Tick id="d-610" ht="10px" mb="mb-[6px]" />
        <Tick id="d-611" ht="23px" /><Tick id="d-612" ht="10px" mb="mb-[6px]" />
        <Tick id="d-613" ht="23px" /><Tick id="d-614" ht="10px" mb="mb-[6px]" />
        <Tick id="d-615" ht="23px" /><Tick id="d-616" ht="10px" mb="mb-[6px]" />
        <Tick id="d-617" ht="23px" /><Tick id="d-618" ht="10px" mb="mb-[6px]" />
        <Tick id="d-619" ht="23px" /><Tick id="d-620" ht="10px" mb="mb-[6px]" />
        <Tick id="d-621" ht="23px" /><Tick id="d-622" ht="10px" mb="mb-[6px]" />
        <Tick id="d-623" ht="23px" /><Tick id="d-624" ht="10px" mb="mb-[6px]" />
        <Tick id="d-625" ht="23px" /><Tick id="d-626" ht="10px" mb="mb-[6px]" />
        <Tick id="d-627" ht="23px" /><Tick id="d-628" ht="10px" mb="mb-[6px]" />
        <Tick id="d-629" ht="23px" /><Tick id="d-630" ht="10px" mb="mb-[6px]" />
        <Tick id="d-631" ht="23px" /><Tick id="d-632" ht="10px" mb="mb-[6px]" />
        <Tick id="d-633" ht="23px" /><Tick id="d-634" ht="10px" mb="mb-[6px]" />
        <Tick id="d-635" ht="23px" /><Tick id="d-636" ht="10px" mb="mb-[6px]" />
        <Tick id="d-637" ht="23px" /><Tick id="d-638" ht="10px" mb="mb-[6px]" />
        <Tick id="d-639" ht="23px" />
      </div>
      {/* braces + headline centered in the 698px brace box */}
      <text id="obl" class="absolute font-[k3-sans-light] font-light text-[#dcdcde] leading-none left-[501.722px] top-[473.661px] text-[128.8546px]">{'{'}</text>
      <text id="obr" class="absolute font-[k3-sans-light] font-light text-[#dcdcde] leading-none left-[1362.496px] top-[473.661px] text-[128.8546px]">{'}'}</text>
      <div id="opentext" class="absolute font-[k3-sans] font-normal text-[#d8d8da] leading-none flex whitespace-nowrap left-[590.3768px] top-[511.5263px] text-[64.4305px] tracking-[-1.6px]"><text id="ot1" class="opacity-0">Open Frontier</text><text id="ot2" class="opacity-0">{" Intelligence"}</text></div>
      <text id="openmid" class="absolute font-[k3-sans] font-normal text-[#d8d8da] leading-none whitespace-nowrap left-[666.7227px] top-[511.5263px] text-[64.4305px] tracking-[-1.6px] opacity-0">en Frontier Intelligen</text>
      <text id="openrem" class="absolute font-[k3-sans] font-normal text-[#98989c] leading-none whitespace-nowrap left-[923.6878px] top-[511.5263px] text-[64.4305px] tracking-[-1.6px] opacity-0">ier</text>
    </div>
  );
}
