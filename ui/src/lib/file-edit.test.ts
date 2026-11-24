import { expect, test } from "vitest";
import { fileEditProblem } from "./file-edit";

test("reads the code the backend puts before its sentence", () => {
  expect(fileEditProblem("conflict: The file changed since it was opened. Nothing was written.")).toEqual({
    problem: "conflict",
    message: "The file changed since it was opened. Nothing was written.",
  });
  expect(fileEditProblem(new Error("too_large: The file is too large to edit here.")).problem).toBe("too_large");
  expect(fileEditProblem("not_text: The file is not UTF-8 text.").problem).toBe("not_text");
});

test("treats anything else as another problem and keeps the whole text", () => {
  expect(fileEditProblem("Choose a regular file inside the task's worktree.")).toEqual({
    problem: "other",
    message: "Choose a regular file inside the task's worktree.",
  });
  expect(fileEditProblem("unknown_code: text").problem).toBe("other");
  expect(fileEditProblem(undefined).problem).toBe("other");
});
