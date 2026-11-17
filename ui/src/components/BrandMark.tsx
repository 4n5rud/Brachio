// App icon: the mint brachiosaurus in its dark squircle (assets/brand/icon.svg).
export const NECK =
  "M 318 1260 C 380 1000, 420 760, 470 620 C 520 470, 610 382, 745 380 C 860 378, 925 450, 920 560 C 915 650, 820 700, 740 708 C 705 712, 695 740, 693 790 C 690 900, 710 1100, 738 1260 Z";

export function BrandMark(props: { class?: string }) {
  return (
    <svg class={props.class ?? "brand-mark"} viewBox="0 0 1024 1024" aria-hidden="true">
      <rect width="1024" height="1024" rx="230" fill="#1F1F24" />
      <g transform="scale(0.9184) translate(-70 -70)">
        <path d={NECK} fill="#98E0BF" />
        <ellipse cx="680" cy="525" rx="25" ry="40" fill="#1F1F24" />
        <ellipse cx="836" cy="525" rx="25" ry="40" fill="#1F1F24" />
      </g>
    </svg>
  );
}
