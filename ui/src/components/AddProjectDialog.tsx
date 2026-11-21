import { Show, createSignal, onCleanup, onMount } from "solid-js";
import { open as openFolder } from "@tauri-apps/plugin-dialog";
import { t } from "../i18n";
import { cloneUrlProblem, folderNameForUrl } from "../lib/clone-url";
import { cancelClone, cloneProject, initProject, watchCloneProgress, type RepoRecord } from "../lib/workspace";
import { Button } from "./ui/Button";
import { Dialog } from "./ui/Dialog";
import { Input } from "./ui/Input";

const PARENT_KEY = "brachio.projectParent";

function savedParent(): string {
  try {
    return localStorage.getItem(PARENT_KEY) ?? "";
  } catch {
    return "";
  }
}

const URL_PROBLEM = {
  empty: "project.urlEmpty",
  scheme: "project.urlScheme",
  format: "project.urlFormat",
} as const;

/** Clone a repository from a URL, or start an empty project: where it goes and what the folder is called. */
export function AddProjectDialog(props: {
  mode: "clone" | "create";
  onAdded: (project: RepoRecord) => void;
  onClose: () => void;
}) {
  const cloning = () => props.mode === "clone";
  const [url, setUrl] = createSignal("");
  const [parent, setParent] = createSignal(savedParent());
  const [name, setName] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [percent, setPercent] = createSignal(0);
  const [urlShown, setUrlShown] = createSignal(false);
  const [error, setError] = createSignal<string>();
  let jobId = "";
  let first!: HTMLInputElement;
  onMount(() => first.focus());
  onCleanup(() => {
    if (busy() && jobId) void cancelClone(jobId).catch(() => undefined);
  });

  const urlError = () => {
    const problem = cloneUrlProblem(url());
    return cloning() && urlShown() && problem ? t(URL_PROBLEM[problem]) : undefined;
  };
  const folder = () => name().trim() || (cloning() ? folderNameForUrl(url()) : "");
  const ready = () => !busy() && parent().trim() !== "" && (cloning() ? cloneUrlProblem(url()) === null : folder() !== "");

  const choose = async () => {
    try {
      const picked = await openFolder({ directory: true, multiple: false, defaultPath: parent() || undefined });
      if (typeof picked === "string") setParent(picked);
    } catch (failure) {
      setError(String(failure));
    }
  };
  const stop = () => {
    if (jobId) void cancelClone(jobId).catch(() => undefined);
  };
  const requestClose = () => {
    if (busy() && cloning()) stop();
    else if (!busy()) props.onClose();
  };

  const submit = async () => {
    setUrlShown(true);
    if (!ready()) return;
    setBusy(true);
    setError(undefined);
    setPercent(0);
    jobId = crypto.randomUUID();
    const unwatch = cloning() ? await watchCloneProgress(jobId, setPercent).catch(() => undefined) : undefined;
    try {
      const project = cloning()
        ? await cloneProject(jobId, url().trim(), parent().trim(), name().trim() || undefined)
        : await initProject(parent().trim(), folder());
      try {
        localStorage.setItem(PARENT_KEY, parent().trim());
      } catch {
        // The location is only a convenience for next time.
      }
      props.onAdded(project);
    } catch (failure) {
      const message = String(failure);
      if (message !== "cancelled") setError(message);
      setBusy(false);
    } finally {
      unwatch?.();
    }
  };

  const verb = () => (cloning() ? t("project.clone") : t("project.create"));
  const working = () => (cloning() ? t("project.cloningPercent").replace("{percent}", String(percent())) : t("project.creating"));

  return (
    <Dialog
      title={cloning() ? t("project.cloneTitle") : t("project.createTitle")}
      onClose={requestClose}
      escHint
      initialFocus={() => first}
      footer={
        <>
          <Button variant="ghost" disabled={busy() && !cloning()} onClick={requestClose}>
            {busy() && cloning() ? t("project.stopClone") : t("newTask.cancel")}
          </Button>
          <Button variant="primary" disabled={!ready()} loading={busy()} loadingLabel={working()} onClick={() => void submit()}>
            {verb()}
          </Button>
        </>
      }
    >
      <div class="form-field add-project" onKeyDown={(event) => {
        if (event.key === "Enter" && !event.isComposing && event.target instanceof HTMLInputElement) void submit();
      }}>
        <Show when={cloning()}
          fallback={
            <Input ref={first} label={t("project.projectName")} value={name()} disabled={busy()}
              onInput={(event) => setName(event.currentTarget.value)} />
          }>
          <Input ref={first} mono label={t("project.url")} value={url()} disabled={busy()} placeholder="https://github.com/owner/repo.git"
            error={urlError()} onBlur={() => setUrlShown(true)} onInput={(event) => setUrl(event.currentTarget.value)} />
        </Show>
        <div class="location-row">
          <Input mono label={t("project.location")} value={parent()} disabled={busy()}
            error={urlShown() && parent().trim() === "" ? t("project.locationEmpty") : undefined}
            onInput={(event) => setParent(event.currentTarget.value)} />
          <Button disabled={busy()} onClick={() => void choose()}>{t("project.locationChoose")}</Button>
        </div>
        <Show when={cloning()}>
          <Input mono label={t("project.folderName")} hint={t("project.folderNameHint")} value={name()} disabled={busy()}
            placeholder={folderNameForUrl(url())} onInput={(event) => setName(event.currentTarget.value)} />
        </Show>
      </div>
      <Show when={error()}>
        {(message) => (
          <div class="create-failed" role="alert">
            <strong>{cloning() ? t("project.cloneFailed") : t("project.createFailed")}</strong>
            <pre class="create-log">{message()}</pre>
            <p class="form-hint">{t("project.addFailedNote")}</p>
          </div>
        )}
      </Show>
    </Dialog>
  );
}
