import { repoSlug } from "./clone-url";

/** The ways to bring a project in, or to start without one; the combobox footer and the sidebar "+" offer the same ones. */
export type AddProjectKind = "folder" | "clone" | "create" | "scratch";

/** A project as the new task dialog lists it. */
export interface ProjectChoice {
  id: string;
  name: string;
  path: string;
  isGit: boolean;
  /** `origin` URL, when the project has one. */
  remote?: string;
}

/** The muted second line of a project row: `owner/repo` when there is a remote, else the path. */
export function projectSubline(project: ProjectChoice): string {
  return repoSlug(project.remote) ?? project.path;
}

/** Projects whose name, `owner/repo` or path contains the typed text, in their given order. */
export function filterProjects(projects: readonly ProjectChoice[], text: string): ProjectChoice[] {
  const needle = text.trim().toLowerCase();
  if (needle === "") return [...projects];
  return projects.filter((project) =>
    [project.name, projectSubline(project)].some((field) => field.toLowerCase().includes(needle)));
}
