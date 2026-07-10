import { describe, it, expect } from "vitest";
import { renderMarkdown } from "./markdown";

describe("renderMarkdown", () => {
  it("渲染标题", () => {
    expect(renderMarkdown("# 标题")).toContain("<h1>");
  });

  it("渲染无序列表", () => {
    expect(renderMarkdown("- 甲\n- 乙")).toContain("<ul>");
  });

  it("渲染有序列表结构(<ol>)", () => {
    const html = renderMarkdown("1. 第一\n2. 第二");
    expect(html).toContain("<ol>");
    expect(html).toContain("第一");
    expect(html).toContain("第二");
  });

  it("渲染加粗", () => {
    expect(renderMarkdown("**重点**")).toContain("<strong>");
  });

  it("XSS:清理 <script> 及其内容", () => {
    const html = renderMarkdown("<script>alert(1)</script>正文");
    expect(html).not.toContain("<script>");
    expect(html).not.toContain("alert");
    expect(html).toContain("正文");
  });

  it("空字符串不抛错", () => {
    expect(typeof renderMarkdown("")).toBe("string");
  });

  it("中文与特殊字符正常渲染", () => {
    const html = renderMarkdown("# 今日 🎉\n- 完成A/B 测试");
    expect(html).toContain("今日");
    expect(html).toContain("A/B");
    expect(html).toContain("<h1>");
  });
});
