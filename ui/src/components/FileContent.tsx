import { For, Match, Show, Switch, createSignal, type JSX } from "solid-js";
import { t } from "../i18n";
import { highlight, isMarkdown } from "../lib/file-highlight";
import { parseMarkdown, remoteImage, safeLink, type Block, type Inline } from "../lib/markdown-blocks";
import { openExternalLink } from "../lib/pr-status";
import "./FileContent.css";

function RemoteImage(props: { src: string; alt: string }) {
  const [loaded, setLoaded] = createSignal(false);
  return (
    <Show when={remoteImage(props.src)} fallback={<span class="md-muted">{props.alt || props.src}</span>}>
      <Show when={loaded()} fallback={
        <button type="button" class="md-image-gate" onClick={() => setLoaded(true)}>
          {t("files.loadImage")}{props.alt ? `: ${props.alt}` : ""}
        </button>
      }>
        <img class="md-image" src={props.src} alt={props.alt} referrerpolicy="no-referrer" />
      </Show>
    </Show>
  );
}

function Inlines(props: { items: readonly Inline[] }): JSX.Element {
  return (
    <For each={props.items}>{(item) => (
      <Switch>
        <Match when={item.kind === "text" && item}>{(node) => <>{node().text}</>}</Match>
        <Match when={item.kind === "code" && item}>{(node) => <code>{node().text}</code>}</Match>
        <Match when={item.kind === "strong" && item}>{(node) => <strong><Inlines items={node().children} /></strong>}</Match>
        <Match when={item.kind === "em" && item}>{(node) => <em><Inlines items={node().children} /></em>}</Match>
        <Match when={item.kind === "image" && item}>{(node) => <RemoteImage src={node().src} alt={node().alt} />}</Match>
        <Match when={item.kind === "link" && item}>{(node) => (
          <Show when={safeLink(node().href)} fallback={<Inlines items={node().children} />}>
            <a href={node().href} rel="noreferrer noopener" onClick={(event) => {
              event.preventDefault();
              void openExternalLink(node().href).catch(() => {});
            }}><Inlines items={node().children} /></a>
          </Show>
        )}</Match>
      </Switch>
    )}</For>
  );
}

function BlockView(props: { block: Block }) {
  const block = () => props.block;
  return (
    <Switch>
      <Match when={block().kind === "heading" && (block() as Extract<Block, { kind: "heading" }>)}>
        {(node) => <p class={`md-heading md-h${Math.min(node().level, 3)}`}><Inlines items={node().children} /></p>}
      </Match>
      <Match when={block().kind === "paragraph" && (block() as Extract<Block, { kind: "paragraph" }>)}>
        {(node) => <p><Inlines items={node().children} /></p>}
      </Match>
      <Match when={block().kind === "code" && (block() as Extract<Block, { kind: "code" }>)}>
        {(node) => <pre class="file-code">{node().text}</pre>}
      </Match>
      <Match when={block().kind === "quote" && (block() as Extract<Block, { kind: "quote" }>)}>
        {(node) => <blockquote><Inlines items={node().children} /></blockquote>}
      </Match>
      <Match when={block().kind === "list" && (block() as Extract<Block, { kind: "list" }>)}>
        {(node) => (
          <Show when={node().ordered} fallback={<ul><For each={node().items}>{(item) => <li><Inlines items={item} /></li>}</For></ul>}>
            <ol><For each={node().items}>{(item) => <li><Inlines items={item} /></li>}</For></ol>
          </Show>
        )}
      </Match>
      <Match when={block().kind === "rule"}><hr /></Match>
    </Switch>
  );
}

/** Read-only text of a previewed file: Markdown as a document, other files with light syntax colour. */
export function FileContent(props: { path: string; text: string }) {
  return (
    <Show when={isMarkdown(props.path)} fallback={
      <pre class="file-code"><For each={highlight(props.text, props.path)}>{(token) => (
        token.kind === "plain" ? <>{token.text}</> : <span class={`tok-${token.kind}`}>{token.text}</span>
      )}</For></pre>
    }>
      <div class="md-doc"><For each={parseMarkdown(props.text)}>{(block) => <BlockView block={block} />}</For></div>
    </Show>
  );
}
