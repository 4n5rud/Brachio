import { render } from "solid-js/web";
import { App } from "./App";
import "./styles/app.css";

// The title bar leaves room for the macOS traffic lights only on macOS.
const platform = navigator.userAgent.includes("Mac") ? "macos" : "other";
document.documentElement.dataset.platform = platform;

const root = document.getElementById("root");
if (!root) throw new Error("#root element missing in index.html");
render(() => <App />, root);
