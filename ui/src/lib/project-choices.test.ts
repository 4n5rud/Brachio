import { describe, expect, it } from "vitest";
import { filterProjects, projectSubline, type ProjectChoice } from "./project-choices";

const projects: ProjectChoice[] = [
  { id: "1", name: "brachio", path: "/work/brachio", isGit: true, remote: "git@github.com:owner/brachio.git" },
  { id: "2", name: "notes", path: "/home/me/notes", isGit: false },
  { id: "3", name: "api", path: "/work/api", isGit: true, remote: "https://github.com/acme/api-server.git" },
];

describe("projectSubline", () => {
  it("shows owner/repo for a project with a remote and the path otherwise", () => {
    expect(projectSubline(projects[0]!)).toBe("owner/brachio");
    expect(projectSubline(projects[1]!)).toBe("/home/me/notes");
  });
});

describe("filterProjects", () => {
  it("lists everything for empty text and keeps the order", () => {
    expect(filterProjects(projects, "  ").map((project) => project.id)).toEqual(["1", "2", "3"]);
  });

  it("matches the name, the owner/repo and the path, ignoring case", () => {
    expect(filterProjects(projects, "NOTES").map((project) => project.id)).toEqual(["2"]);
    expect(filterProjects(projects, "acme").map((project) => project.id)).toEqual(["3"]);
    expect(filterProjects(projects, "/home/me").map((project) => project.id)).toEqual(["2"]);
    expect(filterProjects(projects, "zzz")).toEqual([]);
  });
});
