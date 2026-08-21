import type { CustomTheme, ThemeConfig } from './bindings';

/**
 * 主题纯数据/纯函数层(注册表 + 预设 + 换算 + 命名)。
 * 唯一 DOM 出口是 applyTheme;其余函数保持纯,便于日后补测。
 */

/** colors 键 = CSS 变量名(不带 --),值 = "#rrggbb"。与 Rust CustomTheme.colors 对齐。 */
export type ThemeColors = Record<string, string>;

export const PRESET_ID = 'editorial-paper';
/** 预设主题显示名(设计语言仍叫 Editorial Paper,对用户展示中文名)。 */
export const PRESET_NAME = '纸墨';

/** 10 个可定制变量,按语义分组;key 与 app.css :root 的 CSS 变量一一对应。 */
export const THEME_VAR_GROUPS = [
  {
    group: '页面',
    vars: [
      { key: 'paper', label: '页面背景', desc: '应用底色、滚动区背景' },
      { key: 'paper-card', label: '卡片底色', desc: '左右面板与设置卡片背景' },
    ],
  },
  {
    group: '文字',
    vars: [
      { key: 'ink', label: '正文', desc: '正文文字、主按钮底色' },
      { key: 'ink-soft', label: '次级文字', desc: '说明、次级按钮文字' },
      { key: 'ink-faint', label: '弱化文字', desc: '占位符、计数、标签' },
    ],
  },
  {
    group: '线条',
    vars: [
      { key: 'line', label: '边框线', desc: '卡片边框、分隔线' },
      { key: 'line-strong', label: '强调边框', desc: '输入框边框、滚动条' },
    ],
  },
  {
    group: '强调',
    vars: [
      { key: 'accent', label: '强调色', desc: '强调按钮、选中态、光标选区' },
      { key: 'accent-2', label: '强调悬停色', desc: '强调按钮 hover' },
    ],
  },
  {
    group: '状态',
    vars: [{ key: 'ok', label: '成功色', desc: '成功提示、完成标记' }],
  },
] as const;

/** 全部变量 key(展平注册表,保持注册表声明顺序)。 */
export const THEME_VAR_KEYS: string[] = THEME_VAR_GROUPS.flatMap((g) => g.vars.map((v) => v.key));

/**
 * 预设值,与 src/app.css :root 逐一对应 —— 改任一侧须同步另一侧。
 * 运行时以 applyTheme 为唯一出口统一显式写入这 10 个变量;
 * :root 样式表仅负责首帧(无 JS 介入时的正确底色)。
 */
export const EDITORIAL_PAPER: ThemeColors = {
  paper: '#f3eee3',
  'paper-card': '#fffdf7',
  ink: '#1f1c18',
  'ink-soft': '#5d564c',
  'ink-faint': '#a59c8d',
  line: '#e7dfd0',
  'line-strong': '#d3c8b4',
  accent: '#9c3a26',
  'accent-2': '#b9492f',
  ok: '#3d6b4e',
};

/**
 * 解析主题的完整颜色集:命中自定义主题时逐 key 合并
 * (缺 key 回退 EDITORIAL_PAPER,多余 key 忽略);未命中/空 → 预设副本。
 * id 缺省取 themeConfig.activeId。
 */
export function resolveColors(
  themeConfig: ThemeConfig | undefined | null,
  id?: string,
): ThemeColors {
  const target = id ?? themeConfig?.activeId ?? '';
  const found = themeConfig?.custom.find((t) => t.id === target);
  if (!found) return { ...EDITORIAL_PAPER };
  const out: ThemeColors = { ...EDITORIAL_PAPER };
  for (const key of THEME_VAR_KEYS) {
    const v = found.colors[key];
    if (typeof v === 'string' && v) out[key] = v;
  }
  return out;
}

/**
 * 把一组颜色应用到全局(唯一 DOM 出口):10 个变量全部显式 setProperty,
 * 不做 removeProperty 分支 —— 运行时单一代码路径,规避预设双源漂移。
 */
export function applyTheme(colors: ThemeColors): void {
  const root = document.documentElement;
  for (const key of THEME_VAR_KEYS) {
    root.style.setProperty(`--${key}`, colors[key] ?? EDITORIAL_PAPER[key]);
  }
}

/** "#rgb" / "#rrggbb" / 无 # 前缀 → {r,g,b};非法输入返回 null。 */
export function hexToRgb(hex: string): { r: number; g: number; b: number } | null {
  let h = hex.trim().replace(/^#/, '');
  if (/^[0-9a-fA-F]{3}$/.test(h)) {
    h = h
      .split('')
      .map((c) => c + c)
      .join('');
  }
  if (!/^[0-9a-fA-F]{6}$/.test(h)) return null;
  return {
    r: parseInt(h.slice(0, 2), 16),
    g: parseInt(h.slice(2, 4), 16),
    b: parseInt(h.slice(4, 6), 16),
  };
}

/** RGB → "#rrggbb"(小写)。各通道四舍五入并钳制到 0-255。 */
export function rgbToHex(r: number, g: number, b: number): string {
  const c = (n: number) => {
    const v = Math.round(Number.isFinite(n) ? n : 0);
    return Math.min(255, Math.max(0, v)).toString(16).padStart(2, '0');
  };
  return `#${c(r)}${c(g)}${c(b)}`;
}

/** 新主题自动名:"自定义主题 N",N = 未被占用的最小正整数(与现有 name 全等比较)。 */
export function nextThemeName(custom: CustomTheme[]): string {
  const used = new Set(custom.map((t) => t.name));
  for (let n = 1; ; n++) {
    const name = `自定义主题 ${n}`;
    if (!used.has(name)) return name;
  }
}

/** 重命名冲突时追加 " (2)" 递增后缀;不冲突原样返回。调用方负责排除被重命名者自身。 */
export function dedupeName(name: string, custom: CustomTheme[]): string {
  const used = new Set(custom.map((t) => t.name));
  if (!used.has(name)) return name;
  for (let n = 2; ; n++) {
    const cand = `${name} (${n})`;
    if (!used.has(cand)) return cand;
  }
}

/**
 * 导出为分享 JSON(紧凑):仅 name + colors 两个字段,id 不参与(导入方永远新造)。
 * colors 只输出 THEME_VAR_KEYS 白名单内的键 —— 存储层可能有多余键,导出即规范化。
 */
export function exportThemeJson(theme: CustomTheme): string {
  const colors: ThemeColors = {};
  for (const key of THEME_VAR_KEYS) {
    const v = theme.colors[key];
    if (typeof v === 'string' && v) colors[key] = v;
  }
  return JSON.stringify({ name: theme.name, colors });
}

/**
 * 解析分享 JSON,全量防御,任一不过 → null(错误提示留给 UI):
 * 1. JSON.parse 失败;
 * 2. name 非非空 string(空白视为空,不静默改名);
 * 3. colors 非普通对象;
 * 4. colors 按 THEME_VAR_KEYS 白名单 + hexToRgb 合法性逐 key 过滤,全非法(无任何合法色)。
 * 返回 trim 后的 name 与过滤后的 colors;缺 key 不在此补全 —— resolveColors 已兜底回退预设。
 */
export function parseThemeJson(text: string): { name: string; colors: ThemeColors } | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    return null;
  }
  if (typeof parsed !== 'object' || parsed === null) return null;
  const { name, colors } = parsed as { name?: unknown; colors?: unknown };
  if (typeof name !== 'string' || !name.trim()) return null;
  if (typeof colors !== 'object' || colors === null || Array.isArray(colors)) return null;
  const out: ThemeColors = {};
  for (const key of THEME_VAR_KEYS) {
    const v = (colors as Record<string, unknown>)[key];
    if (typeof v === 'string' && hexToRgb(v) !== null) out[key] = v;
  }
  if (Object.keys(out).length === 0) return null;
  return { name: name.trim(), colors: out };
}
