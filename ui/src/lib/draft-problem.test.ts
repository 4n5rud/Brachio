import { beforeEach, expect, test } from "vitest";
import { clearDraftProblem, noteDraftProblem, takeDraftProblem } from "./draft-problem";

beforeEach(clearDraftProblem);

test("says why the draft did not go in, once", () => {
  noteDraftProblem("working");
  expect(takeDraftProblem()).toBe("The agent is working. Try again when it is idle.");
  expect(takeDraftProblem()).toBe("The agent prompt is unavailable. Check the terminal.");
});

test("reports nothing when the person cancelled", () => {
  noteDraftProblem("cancelled");
  expect(takeDraftProblem()).toBeUndefined();
});
