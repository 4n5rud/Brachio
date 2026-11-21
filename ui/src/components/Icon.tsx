import type { JSX } from "solid-js";

// 16px stroke icons drawn for Brachio. Decorative by default; give the
// surrounding button an aria-label when the icon is the only content.
// Each entry is a function: a DOM node can live in only one place, so every
// use needs fresh nodes.
const PATHS = {
  search: () => <><circle cx="7" cy="7" r="4.5" /><path d="M10.5 10.5 14 14" /></>,
  plus: () => <path d="M8 3v10M3 8h10" />,
  bell: () => <><path d="M4 11V7.2a4 4 0 0 1 8 0V11l1.2 1.5H2.8z" /><path d="M6.6 14.2h2.8" /></>,
  sliders: () => <><path d="M2.5 4.5h11M2.5 8h11M2.5 11.5h11" /><circle cx="6" cy="4.5" r="1.4" fill="none" /><circle cx="10.5" cy="8" r="1.4" fill="none" /><circle cx="5" cy="11.5" r="1.4" fill="none" /></>,
  branch: () => <><circle cx="4.5" cy="3.5" r="1.5" /><circle cx="4.5" cy="12.5" r="1.5" /><circle cx="11.5" cy="4.5" r="1.5" /><path d="M4.5 5v6M11.5 6c0 3.2-7 2.4-7 5" /></>,
  terminal: () => <><rect x="2" y="3" width="12" height="10" rx="1.5" /><path d="m5 6.8 2 1.6-2 1.6M8.5 10.5H11" /></>,
  board: () => <><rect x="2.5" y="2.5" width="4.5" height="4.5" rx="1" /><rect x="9" y="2.5" width="4.5" height="4.5" rx="1" /><rect x="2.5" y="9" width="4.5" height="4.5" rx="1" /><rect x="9" y="9" width="4.5" height="4.5" rx="1" /></>,
  list: () => <><path d="M5.5 4h8M5.5 8h8M5.5 12h8" /><circle cx="2.8" cy="4" r=".6" fill="currentColor" /><circle cx="2.8" cy="8" r=".6" fill="currentColor" /><circle cx="2.8" cy="12" r=".6" fill="currentColor" /></>,
  diff: () => <><path d="M5 2.5v5M2.5 5h5M9 12h4.5" /><path d="M11 2.5 5 13.5" opacity=".6" /></>,
  check: () => <path d="m3.5 8.5 3 3 6-7" />,
  x: () => <path d="m4 4 8 8M12 4l-8 8" />,
  globe: () => <><circle cx="8" cy="8" r="5.5" /><path d="M2.5 8h11" /><ellipse cx="8" cy="8" rx="2.4" ry="5.5" /></>,
  chevronDown: () => <path d="m4 6 4 4 4-4" />,
  chevronRight: () => <path d="m6 4 4 4-4 4" />,
  pr: () => <><circle cx="4" cy="3.5" r="1.5" /><circle cx="4" cy="12.5" r="1.5" /><circle cx="12" cy="12.5" r="1.5" /><path d="M4 5v6M12 11V7a2 2 0 0 0-2-2H7.5" /><path d="M9 3.4 7.4 5 9 6.6" /></>,
  cpu: () => <><rect x="4.5" y="4.5" width="7" height="7" rx="1" /><path d="M6.5 2v2M9.5 2v2M6.5 12v2M9.5 12v2M2 6.5h2M2 9.5h2M12 6.5h2M12 9.5h2" /></>,
  memory: () => <><rect x="2" y="4.5" width="12" height="7" rx="1" /><path d="M5 11.5v2M8 11.5v2M11 11.5v2" /></>,
  more: () => <><circle cx="3.5" cy="8" r=".9" fill="currentColor" /><circle cx="8" cy="8" r=".9" fill="currentColor" /><circle cx="12.5" cy="8" r=".9" fill="currentColor" /></>,
  split: () => <><rect x="2" y="3" width="12" height="10" rx="1.5" /><path d="M8 3v10" /></>,
  external: () => <path d="M9.5 2.5h4v4M13.5 2.5 8 8M11.5 9.5v4h-9v-9h4" />,
  folder: () => <path d="M2 4.5h4l1.5 1.5H14v6.5H2z" />,
  panelLeft: () => <><rect x="2" y="3" width="12" height="10" rx="1.5" /><path d="M6 3v10" /></>,
  panelRight: () => <><rect x="2" y="3" width="12" height="10" rx="1.5" /><path d="M10 3v10" /></>,
  download: () => <><path d="M8 2.5v7M5 6.5l3 3 3-3" /><path d="M2.5 10.5v2a1 1 0 0 0 1 1h9a1 1 0 0 0 1-1v-2" /></>,
  mail: () => <><rect x="2" y="3.5" width="12" height="9" rx="1.5" /><path d="m2.5 4.5 5.5 4.2 5.5-4.2" /></>,
  keyboard: () => <><rect x="2" y="4" width="12" height="8" rx="1.5" /><path d="M5 7h.01M8 7h.01M11 7h.01M5.5 9.5h5" /></>,
  refresh: () => <><path d="M13 8a5 5 0 1 1-1.5-3.6" /><path d="M13 2.5v2.8h-2.8" /></>,
  bot: () => <><rect x="3" y="5" width="10" height="8" rx="2" /><path d="M8 2.5V5M6 9h.01M10 9h.01" /></>,
} satisfies Record<string, () => JSX.Element>;

export type IconName = keyof typeof PATHS;

export function Icon(props: { name: IconName; class?: string }) {
  return (
    <svg
      class={props.class ?? "icon"}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      stroke-width="1.5"
      stroke-linecap="round"
      stroke-linejoin="round"
      aria-hidden="true"
    >
      {PATHS[props.name]()}
    </svg>
  );
}
