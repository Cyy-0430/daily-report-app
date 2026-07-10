import { describe, it, expect } from "vitest";
import { DEFAULT_PROMPT_TEMPLATE } from "./template";

describe("DEFAULT_PROMPT_TEMPLATE", () => {
  it("非空", () => {
    expect(DEFAULT_PROMPT_TEMPLATE.length).toBeGreaterThan(0);
  });

  it("包含 render_template 所需占位符", () => {
    expect(DEFAULT_PROMPT_TEMPLATE).toContain("{{date}}");
    expect(DEFAULT_PROMPT_TEMPLATE).toContain("{{input}}");
    expect(DEFAULT_PROMPT_TEMPLATE).toContain("{{conversations}}");
  });
});
