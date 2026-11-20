import { For, Show, createSignal } from "solid-js";
import { render } from "solid-js/web";
import "../styles/app.css";
import "./catalog.css";
import { locale, setLocale, t } from "../i18n";
import { COLOR_THEMES } from "../lib/theme";
import { pushToast } from "../lib/toasts";
import { Button } from "../components/ui/Button";
import { ConfirmHost, confirmDialog } from "../components/ui/ConfirmDialog";
import { Dialog } from "../components/ui/Dialog";
import { EmptyState } from "../components/ui/EmptyState";
import { IconButton } from "../components/ui/IconButton";
import { Input, Select, TextArea } from "../components/ui/Input";
import { Kbd } from "../components/ui/Kbd";
import { Notice } from "../components/ui/Notice";
import { SegmentedControl } from "../components/ui/SegmentedControl";
import { Toast, ToastHost } from "../components/ui/Toast";
import { Toggle } from "../components/ui/Toggle";

// Dev only: open /catalog.html on the Vite dev server. It is not an input of `vite build`.
if (!import.meta.env.DEV) throw new Error("The component catalog runs only on the dev server.");

const THEMES = COLOR_THEMES.flatMap((color) => (["dark", "light"] as const).map((mode) => `${color}-${mode}`));

function Row(props: { cap: string; children: import("solid-js").JSX.Element }) {
  return (
    <div class="catalog-row">
      <span class="catalog-cap">{props.cap}</span>
      {props.children}
    </div>
  );
}

function Section(props: { title: string; children: import("solid-js").JSX.Element }) {
  return (
    <section>
      <h2>{props.title}</h2>
      <div class="catalog-panel">{props.children}</div>
    </section>
  );
}

function Catalog() {
  const [theme, setTheme] = createSignal("mint-dark");
  const [mode, setMode] = createSignal<"tasks" | "board">("tasks");
  const [notify, setNotify] = createSignal(true);
  const [dialog, setDialog] = createSignal<"plain" | "confirm" | undefined>();
  const applyTheme = (value: string) => {
    setTheme(value);
    document.documentElement.dataset.theme = value;
  };
  document.documentElement.dataset.theme = theme();
  const states = [
    { cap: "default", class: undefined },
    { cap: "hover", class: "is-hover" },
    { cap: "pressed", class: "is-press" },
    { cap: "focus", class: "is-focus" },
  ] as const;
  const variants = ["secondary", "primary", "ghost", "danger"] as const;

  return (
    <main class="catalog">
      <header class="catalog-bar">
        <h1>Brachio components</h1>
        <Select aria-label="Theme" value={theme()} onChange={(event) => applyTheme(event.currentTarget.value)}>
          <For each={THEMES}>{(name) => <option value={name}>{name}</option>}</For>
        </Select>
        <SegmentedControl label="Language" value={locale()} onChange={setLocale}
          options={[{ value: "en", label: "English" }, { value: "ko", label: "한국어" }]} />
      </header>

      <Section title="Button">
        <For each={variants}>{(variant) => (
          <Row cap={variant}>
            <For each={states}>{(state) => (
              <Button variant={variant} class={state.class}>{t("task.archive")}</Button>
            )}</For>
            <Button variant={variant} disabled>{t("task.archive")}</Button>
            <Button variant={variant} loading loadingLabel={t("newTask.creating")}>{t("task.archive")}</Button>
          </Row>
        )}</For>
        <Row cap="small">
          <Button size="sm">{t("inspector.refresh")}</Button>
          <Button size="sm" variant="primary">{t("task.restore")}</Button>
          <Button size="sm" variant="ghost">{t("workspace.close")}</Button>
        </Row>
        <Row cap="icon, kbd">
          <Button icon="folder">{t("empty.repo.add")}</Button>
          <Button variant="primary" icon="plus" kbd="Ctrl+Enter">{t("newTask.create")}</Button>
        </Row>
      </Section>

      <Section title="IconButton">
        <Row cap="states">
          <For each={states}>{(state) => (
            <IconButton icon="split" label={t("task.split")} class={state.class} />
          )}</For>
          <IconButton icon="split" label={t("task.split")} disabled />
          <IconButton icon="split" label={t("task.split")} aria-pressed={true} />
          <IconButton icon="bell" label="Activity" badge={3} />
          <IconButton icon="plus" size="sm" label={t("task.newTab")} />
        </Row>
      </Section>

      <Section title="Input">
        <div class="catalog-fields">
          <Input label="Default" placeholder="Example value" />
          <Input label="Focus" class="is-focus" value="Focused" />
          <Input label="Mono" mono value="feature/login" hint="A branch name" />
          <Input label="Error" value="mian" error="No branch named mian. Pick one from the list." />
          <Input label="Disabled" value="Locked" disabled />
          <Select label="Select"><option>One</option><option>Two</option></Select>
        </div>
        <TextArea label="Multiline" placeholder="Describe the task" />
      </Section>

      <Section title="Toggle">
        <Toggle checked={notify()} onChange={setNotify} label={t("settings.notifications")} hint="Takes effect at once" />
        <Toggle checked={false} onChange={() => {}} label="Off" />
        <Toggle checked={true} onChange={() => {}} label="On" disabled />
      </Section>

      <Section title="SegmentedControl">
        <Row cap="two">
          <SegmentedControl label="View" value={mode()} onChange={setMode}
            options={[
              { value: "tasks", label: t("view.tasks"), icon: "list" },
              { value: "board", label: t("view.board"), icon: "board" },
            ]} />
        </Row>
        <Row cap="four">
          <SegmentedControl label="Four" value="b" onChange={() => {}}
            options={[{ value: "a", label: "One" }, { value: "b", label: "Two" }, { value: "c", label: "Three" }, { value: "d", label: "Four", disabled: true }]} />
        </Row>
      </Section>

      <Section title="Kbd">
        <Row cap="keys"><Kbd>Ctrl</Kbd><Kbd>Shift</Kbd><Kbd>K</Kbd><Kbd>Esc</Kbd></Row>
      </Section>

      <Section title="Dialog and ConfirmDialog">
        <Row cap="open">
          <Button onClick={() => setDialog("plain")}>Dialog</Button>
          <Button variant="danger" onClick={() => void confirmDialog({
            title: "Delete this task for good?",
            body: "The worktree folder and the branch feature/login are removed.",
            loss: [
              { label: "Uncommitted files", count: 4 },
              { label: "Commits only on this branch", count: 2 },
              { label: "Running terminals", count: 0 },
            ],
            confirmLabel: t("task.delete"),
          })}>Confirm, danger</Button>
          <Button onClick={() => void confirmDialog({
            title: "Archive this task?",
            body: "Nothing is lost. You can restore it later.",
            confirmLabel: t("task.archive"),
            tone: "primary",
          })}>Confirm, primary</Button>
        </Row>
        <Show when={dialog() === "plain"}>
          <Dialog title="Dialog" escHint onClose={() => setDialog(undefined)} closeOnScrim
            footer={<Button variant="primary" onClick={() => setDialog(undefined)}>{t("workspace.close")}</Button>}>
            <Input label="Name" />
          </Dialog>
        </Show>
      </Section>

      <Section title="Toast">
        <Row cap="static">
          <Toast tone="success" message="Pull request opened" onDismiss={() => {}} />
          <Toast tone="info" message={t("updates.available").replace("{version}", "0.2.0")}
            action={{ label: t("updates.download"), onClick: () => {} }} onDismiss={() => {}} />
          <Toast tone="error" title={t("error.title")} message="Push rejected. Pull first, then push again."
            action={{ label: t("error.report"), onClick: () => {} }} onDismiss={() => {}} />
        </Row>
        <Row cap="live">
          <Button onClick={() => pushToast({ tone: "success", message: "Pull request opened" })}>Confirmation (3 s)</Button>
          <Button onClick={() => pushToast({ tone: "error", title: t("error.title"), message: "Could not start the agent." })}>Error (stays)</Button>
        </Row>
      </Section>

      <Section title="Notice">
        <div class="catalog-stack" style={{ width: "100%" }}>
          <Notice tone="info" lead="Last screen recovered.">Output before the restart is skipped.</Notice>
          <Notice tone="progress">{t("terminal.checkingConversations")}</Notice>
          <Notice tone="success" lead="Reattached.">The terminal kept running.</Notice>
          <Notice tone="blocked" lead={t("terminal.launchUncertain")}
            actions={<Button size="sm">{t("terminal.launchReset")}</Button>} />
          <Notice tone="error" lead="Lost the runtime connection."
            actions={<Button size="sm">{t("terminal.reopen")}</Button>}>Reopen the session to continue.</Notice>
        </div>
      </Section>

      <Section title="EmptyState">
        <EmptyState mascot title={t("empty.repo.title")} body={t("empty.repo.body")}>
          <Button variant="primary" icon="folder">{t("empty.repo.add")}</Button>
        </EmptyState>
        <EmptyState title={t("empty.task.title")} body={t("empty.task.body")} />
      </Section>

      <ConfirmHost />
      <ToastHost />
    </main>
  );
}

const root = document.getElementById("root");
if (!root) throw new Error("#root element missing in catalog.html");
render(() => <Catalog />, root);
