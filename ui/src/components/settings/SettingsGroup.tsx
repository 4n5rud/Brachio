import type { JSX } from "solid-js";

/** A run of settings rows on one page, divided by hairlines. */
export function SettingsGroup(props: { children: JSX.Element }) {
  return <div class="settings-group">{props.children}</div>;
}
