import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // markdown 渲染用到 DOMPurify,需要 DOM 环境
    // (用 jsdom 而非 happy-dom:后者 HTML 解析不完整,会吞掉 h1/ol/ul 等块级标签)
    environment: "jsdom",
    include: ["src/**/*.test.ts"],
  },
});
