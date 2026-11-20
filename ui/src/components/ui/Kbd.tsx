import type { JSX } from "solid-js";
import "./Kbd.css";

/** One key cap. The consumer passes the platform's own key names. */
export function Kbd(props: { children: JSX.Element }) {
  return <kbd>{props.children}</kbd>;
}
