import { render } from "solid-js/web";
import { App } from "./App";
import "./styles/app.css";

const root = document.getElementById("root");
if (!root) throw new Error("#root element missing in index.html");
render(() => <App />, root);
