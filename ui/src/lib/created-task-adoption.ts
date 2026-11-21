import type { CreatedTask, RepoRecord } from "./workspace";

/** What the window does with a task that appeared outside its own create dialog. */
export interface AdoptionHost {
  isListed(taskId: string): boolean;
  hasRepo(repoId: string): boolean;
  listRepos(): Promise<readonly RepoRecord[]>;
  showRepos(repos: readonly RepoRecord[]): void;
  markCreatedHere(taskId: string): void;
  failSetupCopy(taskId: string, error: string): void;
  rememberPrompt(taskId: string, prompt: string): void;
  announceChild(task: CreatedTask): void;
  /** Adds the row unless it is listed, and opens the task's tab without moving the selection. */
  open(task: CreatedTask, listed: boolean): void;
  requestSetup(task: CreatedTask): void;
}

/**
 * Starts the tab of a task an agent created or the scheduler started. A task the scheduler starts
 * may already be listed, because it was created earlier and waited; any other listed task is a duplicate.
 */
export async function adoptCreatedTask(host: AdoptionHost, record: CreatedTask, startListed = false) {
  if (host.isListed(record.id) && !startListed) return;
  if (!host.hasRepo(record.repoId)) {
    const stored = await host.listRepos();
    if (!stored.some((item) => item.id === record.repoId)) return;
    host.showRepos(stored);
    if (host.isListed(record.id) && !startListed) return;
  }
  host.markCreatedHere(record.id);
  if (record.setupCopyError) host.failSetupCopy(record.id, record.setupCopyError);
  if (record.prompt) host.rememberPrompt(record.id, record.prompt);
  const listed = host.isListed(record.id);
  if (listed && !startListed) return;
  if (!listed && record.parentTaskId) host.announceChild(record);
  host.open(record, listed);
  host.requestSetup(record);
}
