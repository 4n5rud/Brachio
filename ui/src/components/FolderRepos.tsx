import { Button } from "./ui/Button";
import { For, Show, createResource, createSignal } from "solid-js";
import { t } from "../i18n";
import { listFolderRepos } from "../lib/workspace";

/**
 * Git repositories found inside a folder project, so each can become a project of its own. A folder
 * such as a workspace root holds several; the folder project itself has no git features.
 */
export function FolderRepos(props: {
  projectId: string;
  onAdd: (path: string) => Promise<void>;
}) {
  const [repos, { refetch }] = createResource(
    () => props.projectId,
    async (id) => {
      try {
        return await listFolderRepos(id);
      } catch {
        return [];
      }
    },
  );
  const [adding, setAdding] = createSignal<string>();
  const add = async (path: string) => {
    setAdding(path);
    try {
      await props.onAdd(path);
    } finally {
      setAdding(undefined);
      void refetch();
    }
  };
  return (
    <Show when={(repos() ?? []).length > 0}>
      <div class="folder-repos">
        <p class="folder-repos-title">{t("folderRepos.title")}</p>
        <For each={repos()}>
          {(repo) => (
            <div class="folder-repo">
              <span class="folder-repo-name" title={repo.path}>{repo.name}</span>
              <Button size="sm" busy={adding() === repo.path} disabled={repo.added || adding() !== undefined} onClick={() => void add(repo.path)}>
                {repo.added ? t("folderRepos.added") : t("folderRepos.add")}
              </Button>
            </div>
          )}
        </For>
      </div>
    </Show>
  );
}
