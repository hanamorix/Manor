import { describe, it, expect } from "vitest";
import { parseSlash } from "./slash";

describe("parseSlash", () => {
  it("returns null for non-slash input", () => {
    expect(parseSlash("hello")).toBeNull();
    expect(parseSlash("")).toBeNull();
    expect(parseSlash("  /task ")).toBeNull();
  });

  it("parses /task with title", () => {
    expect(parseSlash("/task pick up prescription")).toEqual({
      type: "task",
      title: "pick up prescription",
    });
  });

  it("trims trailing whitespace from title", () => {
    expect(parseSlash("/task  reply to Miriam   ")).toEqual({
      type: "task",
      title: "reply to Miriam",
    });
  });

  it("returns null for /task with empty title", () => {
    expect(parseSlash("/task")).toBeNull();
    expect(parseSlash("/task   ")).toBeNull();
  });

  it("returns unknown for unrecognised slash command", () => {
    expect(parseSlash("/banana split")).toEqual({
      type: "unknown",
      raw: "/banana split",
    });
  });
});
