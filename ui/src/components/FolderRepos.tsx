import { For, Show, createResource } from "solid-js";
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
  const add = async (path: string) => {
    try {
      await props.onAdd(path);
    } finally {
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
              <button type="button" class="button button-sm" disabled={repo.added} onClick={() => void add(repo.path)}>
                {repo.added ? t("folderRepos.added") : t("folderRepos.add")}
              </button>
            </div>
          )}
        </For>
      </div>
    </Show>
  );
}
