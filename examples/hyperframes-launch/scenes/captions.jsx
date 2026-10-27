import { useGSAP } from 'opencat/runtime';
import { vis } from '../shared.jsx';

/* ══ captions (index.html:242-275) — 14 inline body-level overlays ══════ */
var CAPS = [
  ['cap0', 7.29, 9.15, 'HyperFrames by HeyGen.'],
  ['cap1', 9.10, 12.54, 'Agents can now write HTML and render MP4s.'],
  ['cap2', 12.73, 15.21, 'Claude Code made this launch video.'],
  ['cap3', 15.28, 16.58, 'Your agent can too.'],
  ['cap4', 18.20, 19.30, 'CSS animations'],
  ['cap5', 19.41, 20.34, 'GSAP'],
  ['cap6', 20.44, 21.34, 'Shaders'],
  ['cap7', 29.42, 30.60, 'composes together'],
  ['cap8', 32.74, 34.00, 'Your agent writes code to compose'],
  ['cap9', 34.14, 38.00, 'Hyperframes renders every frame to MP4.'],
  ['cap10', 38.16, 40.46, "It's deterministic pixel-perfect every time"],
  ['cap11', 40.60, 42.31, 'Give your agent the skill.'],
  ['cap12', 42.31, 43.91, 'Tell it what to make,'],
  ['cap13', 43.91, 44.98, 'watch it build'],
];

export default function CaptionLayer() {
  useGSAP((ctx) => {
    var T = ctx.global.time;
    for (var i = 0; i < CAPS.length; i++) vis(ctx, CAPS[i][0], T >= CAPS[i][1] && T < CAPS[i][2]);
  });
  return (
    <div className="absolute left-0 top-0 w-[1920px] h-[1080px] z-[50]">
      {CAPS.map((c) => (
        <div id={c[0]} className="absolute left-0 bottom-[80px] w-full flex justify-center opacity-0">
          <div className="px-[28px] py-[12px] rounded-[10px] bg-[rgba(10,8,5,0.55)] backdrop-blur-[8px] whitespace-nowrap">
            <p className="text-[40px] leading-[48px] tracking-[-0.005em] font-[500] font-[hf-inter-med] text-[rgba(240,235,220,0.92)]">{c[3]}</p>
          </div>
        </div>
      ))}
    </div>
  );
}
