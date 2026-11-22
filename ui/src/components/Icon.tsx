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
  gear: () => <><path d="M6.8 3.1 7 1.5h2l.2 1.6 1.4.6 1.3-1 1.4 1.4-1 1.3.6 1.4 1.6.2v2l-1.6.2-.6 1.4 1 1.3-1.4 1.4-1.3-1-1.4.6-.2 1.6H7l-.2-1.6-1.4-.6-1.3 1-1.4-1.4 1-1.3-.6-1.4-1.6-.2V7l1.6-.2.6-1.4-1-1.3 1.4-1.4 1.3 1z" /><circle cx="8" cy="8" r="2.2" /></>,
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
  folderOpen: () => <path d="M2 12.5V4.5h4l1.5 1.5H13v2M2 12.5l1.8-4.5H14.5l-1.8 4.5z" />,
  file: () => <><path d="M4 2h5l3 3v9H4z" /><path d="M9 2v3h3" /></>,
  panelLeft: () => <><rect x="2" y="3" width="12" height="10" rx="1.5" /><path d="M6 3v10" /></>,
  panelRight: () => <><rect x="2" y="3" width="12" height="10" rx="1.5" /><path d="M10 3v10" /></>,
  archive: () => <><rect x="2" y="3" width="12" height="3" rx="1" /><path d="M3 6v6a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1V6M6.5 9h3" /></>,
  message: () => <path d="M2.5 3.5h11v7h-6l-3 2.5v-2.5h-2z" />,
  checkCircle: () => <><circle cx="8" cy="8" r="5.5" /><path d="m5.5 8.2 1.8 1.8 3.2-3.8" /></>,
  activity: () => <path d="M1.5 8h3l2-5 3 10 2-5h3" />,
  download: () => <><path d="M8 2.5v7M5 6.5l3 3 3-3" /><path d="M2.5 10.5v2a1 1 0 0 0 1 1h9a1 1 0 0 0 1-1v-2" /></>,
  mail: () => <><rect x="2" y="3.5" width="12" height="9" rx="1.5" /><path d="m2.5 4.5 5.5 4.2 5.5-4.2" /></>,
  pin: () => <path d="M6 2.5h4l-.5 4 2 2H4.5l2-2zM8 8.5V14" />,
  keyboard: () => <><rect x="2" y="4" width="12" height="8" rx="1.5" /><path d="M5 7h.01M8 7h.01M11 7h.01M5.5 9.5h5" /></>,
  refresh: () => <><path d="M13 8a5 5 0 1 1-1.5-3.6" /><path d="M13 2.5v2.8h-2.8" /></>,
  bot: () => <><rect x="3" y="5" width="10" height="8" rx="2" /><path d="M8 2.5V5M6 9h.01M10 9h.01" /></>,
  // GitHub Primer Octicons 19.42.0 (MIT, see assets/brand-icons/LICENSE-octicons): filled glyphs for pull request, issue and CI state.
  ghPr: () => <><path fill="currentColor" stroke="none" d="M1.5 3.25a2.25 2.25 0 1 1 3 2.122v5.256a2.251 2.251 0 1 1-1.5 0V5.372A2.25 2.25 0 0 1 1.5 3.25Zm5.677-.177L9.573.677A.25.25 0 0 1 10 .854V2.5h1A2.5 2.5 0 0 1 13.5 5v5.628a2.251 2.251 0 1 1-1.5 0V5a1 1 0 0 0-1-1h-1v1.646a.25.25 0 0 1-.427.177L7.177 3.427a.25.25 0 0 1 0-.354ZM3.75 2.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm0 9.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm8.25.75a.75.75 0 1 0 1.5 0 .75.75 0 0 0-1.5 0Z" /></>,
  ghPrDraft: () => <><path fill="currentColor" stroke="none" d="M3.25 1A2.25 2.25 0 0 1 4 5.372v5.256a2.251 2.251 0 1 1-1.5 0V5.372A2.251 2.251 0 0 1 3.25 1Zm9.5 14a2.25 2.25 0 1 1 0-4.5 2.25 2.25 0 0 1 0 4.5ZM2.5 3.25a.75.75 0 1 0 1.5 0 .75.75 0 0 0-1.5 0ZM3.25 12a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm9.5 0a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5ZM14 7.5a1.25 1.25 0 1 1-2.5 0 1.25 1.25 0 0 1 2.5 0Zm0-4.25a1.25 1.25 0 1 1-2.5 0 1.25 1.25 0 0 1 2.5 0Z" /></>,
  ghMerge: () => <><path fill="currentColor" stroke="none" d="M5.45 5.154A4.25 4.25 0 0 0 9.25 7.5h1.378a2.251 2.251 0 1 1 0 1.5H9.25A5.734 5.734 0 0 1 5 7.123v3.505a2.25 2.25 0 1 1-1.5 0V5.372a2.25 2.25 0 1 1 1.95-.218ZM4.25 13.5a.75.75 0 1 0 0-1.5.75.75 0 0 0 0 1.5Zm8.5-4.5a.75.75 0 1 0 0-1.5.75.75 0 0 0 0 1.5ZM5 3.25a.75.75 0 1 0 0 .005V3.25Z" /></>,
  ghPrClosed: () => <><path fill="currentColor" stroke="none" d="M3.25 1A2.25 2.25 0 0 1 4 5.372v5.256a2.251 2.251 0 1 1-1.5 0V5.372A2.251 2.251 0 0 1 3.25 1Zm9.5 5.5a.75.75 0 0 1 .75.75v3.378a2.251 2.251 0 1 1-1.5 0V7.25a.75.75 0 0 1 .75-.75Zm-2.03-5.273a.75.75 0 0 1 1.06 0l.97.97.97-.97a.748.748 0 0 1 1.265.332.75.75 0 0 1-.205.729l-.97.97.97.97a.751.751 0 0 1-.018 1.042.751.751 0 0 1-1.042.018l-.97-.97-.97.97a.749.749 0 0 1-1.275-.326.749.749 0 0 1 .215-.734l.97-.97-.97-.97a.75.75 0 0 1 0-1.06ZM2.5 3.25a.75.75 0 1 0 1.5 0 .75.75 0 0 0-1.5 0ZM3.25 12a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Zm9.5 0a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Z" /></>,
  ghIssueOpen: () => <><path fill="currentColor" stroke="none" d="M8 9.5a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3Z" /><path fill="currentColor" stroke="none" d="M8 0a8 8 0 1 1 0 16A8 8 0 0 1 8 0ZM1.5 8a6.5 6.5 0 1 0 13 0 6.5 6.5 0 0 0-13 0Z" /></>,
  ghIssueClosed: () => <><path fill="currentColor" stroke="none" d="M11.28 6.78a.75.75 0 0 0-1.06-1.06L7.25 8.69 5.78 7.22a.75.75 0 0 0-1.06 1.06l2 2a.75.75 0 0 0 1.06 0l3.5-3.5Z" /><path fill="currentColor" stroke="none" d="M16 8A8 8 0 1 1 0 8a8 8 0 0 1 16 0Zm-1.5 0a6.5 6.5 0 1 0-13 0 6.5 6.5 0 0 0 13 0Z" /></>,
  ghCheck: () => <><path fill="currentColor" stroke="none" d="M13.78 4.22a.75.75 0 0 1 0 1.06l-7.25 7.25a.75.75 0 0 1-1.06 0L2.22 9.28a.751.751 0 0 1 .018-1.042.751.751 0 0 1 1.042-.018L6 10.94l6.72-6.72a.75.75 0 0 1 1.06 0Z" /></>,
  ghX: () => <><path fill="currentColor" stroke="none" d="M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.749.749 0 0 1 1.275.326.749.749 0 0 1-.215.734L9.06 8l3.22 3.22a.749.749 0 0 1-.326 1.275.749.749 0 0 1-.734-.215L8 9.06l-3.22 3.22a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06Z" /></>,
  ghDot: () => <><path fill="currentColor" stroke="none" d="M8 4a4 4 0 1 1 0 8 4 4 0 0 1 0-8Z" /></>,
  ghComment: () => <><path fill="currentColor" stroke="none" d="M1 2.75C1 1.784 1.784 1 2.75 1h10.5c.966 0 1.75.784 1.75 1.75v7.5A1.75 1.75 0 0 1 13.25 12H9.06l-2.573 2.573A1.458 1.458 0 0 1 4 13.543V12H2.75A1.75 1.75 0 0 1 1 10.25Zm1.75-.25a.25.25 0 0 0-.25.25v7.5c0 .138.112.25.25.25h2a.75.75 0 0 1 .75.75v2.19l2.72-2.72a.749.749 0 0 1 .53-.22h4.5a.25.25 0 0 0 .25-.25v-7.5a.25.25 0 0 0-.25-.25Z" /></>,
  ghBranch: () => <><path fill="currentColor" stroke="none" d="M9.5 3.25a2.25 2.25 0 1 1 3 2.122V6A2.5 2.5 0 0 1 10 8.5H6a1 1 0 0 0-1 1v1.128a2.251 2.251 0 1 1-1.5 0V5.372a2.25 2.25 0 1 1 1.5 0v1.836A2.493 2.493 0 0 1 6 7h4a1 1 0 0 0 1-1v-.628A2.25 2.25 0 0 1 9.5 3.25Zm-6 0a.75.75 0 1 0 1.5 0 .75.75 0 0 0-1.5 0Zm8.25-.75a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5ZM4.25 12a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Z" /></>,
  ghCommit: () => <><path fill="currentColor" stroke="none" d="M11.93 8.5a4.002 4.002 0 0 1-7.86 0H.75a.75.75 0 0 1 0-1.5h3.32a4.002 4.002 0 0 1 7.86 0h3.32a.75.75 0 0 1 0 1.5Zm-1.43-.75a2.5 2.5 0 1 0-5 0 2.5 2.5 0 0 0 5 0Z" /></>,
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
