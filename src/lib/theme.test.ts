import { describe, it, expect } from 'vitest';
import {
  THEME_VAR_KEYS,
  EDITORIAL_PAPER,
  resolveColors,
  hexToRgb,
  rgbToHex,
  nextThemeName,
  dedupeName,
  PRESET_ID,
} from './theme';
import type { CustomTheme } from './bindings';

function mkTheme(id: string, name: string, colors: CustomTheme['colors']): CustomTheme {
  return { id, name, colors };
}

describe('注册表 ↔ 预设互锁', () => {
  it('10 个变量 key,每个都在 EDITORIAL_PAPER 有对应值', () => {
    expect(THEME_VAR_KEYS).toHaveLength(10);
    for (const key of THEME_VAR_KEYS) {
      expect(EDITORIAL_PAPER[key], `key=${key}`).toMatch(/^#[0-9a-f]{6}$/);
    }
  });
});

describe('resolveColors', () => {
  it('config 缺失/空 activeId → 预设副本', () => {
    expect(resolveColors(undefined)).toEqual(EDITORIAL_PAPER);
    expect(resolveColors({ activeId: '', custom: [] })).toEqual(EDITORIAL_PAPER);
  });

  it('activeId 命中自定义主题 → 逐 key 覆盖,缺 key 回退预设,多余 key 忽略', () => {
    const t = mkTheme('a', '自定义主题 1', { paper: '#101010', bogus: '#ff0000' });
    const out = resolveColors({ activeId: 'a', custom: [t] });
    expect(out.paper).toBe('#101010');
    expect(out.ink).toBe(EDITORIAL_PAPER.ink); // 缺 key 回退
    expect(Object.keys(out)).toHaveLength(THEME_VAR_KEYS.length); // 多余 key 不进入
  });

  it('显式 id 优先于 activeId;未命中 id → 预设', () => {
    const a = mkTheme('a', 'A', { ink: '#111111' });
    const cfg = { activeId: 'a', custom: [a] };
    expect(resolveColors(cfg, 'a').ink).toBe('#111111');
    expect(resolveColors(cfg, 'gone')).toEqual(EDITORIAL_PAPER);
    expect(resolveColors(cfg, PRESET_ID)).toEqual(EDITORIAL_PAPER);
  });

  it('返回副本,修改结果不影响预设常量', () => {
    const out = resolveColors({ activeId: '', custom: [] });
    out.paper = '#000000';
    expect(EDITORIAL_PAPER.paper).toBe('#f3eee3');
  });
});

describe('hexToRgb', () => {
  it('6 位 / 3 位展开 / 无 # / 大写 / 首尾空白', () => {
    expect(hexToRgb('#9c3a26')).toEqual({ r: 156, g: 58, b: 38 });
    expect(hexToRgb('#f00')).toEqual({ r: 255, g: 0, b: 0 });
    expect(hexToRgb('1f1c18')).toEqual({ r: 31, g: 28, b: 24 });
    expect(hexToRgb(' #ABCDEF ')).toEqual({ r: 171, g: 205, b: 239 });
  });

  it('非法输入返回 null', () => {
    expect(hexToRgb('#12345')).toBeNull();
    expect(hexToRgb('#1234567')).toBeNull();
    expect(hexToRgb('#gggggg')).toBeNull();
    expect(hexToRgb('')).toBeNull();
  });
});

describe('rgbToHex', () => {
  it('常规换算,输出小写 #rrggbb', () => {
    expect(rgbToHex(156, 58, 38)).toBe('#9c3a26');
    expect(rgbToHex(0, 0, 0)).toBe('#000000');
  });

  it('四舍五入、越界钳制、非有限数按 0', () => {
    expect(rgbToHex(0.6, 0.4, 0)).toBe('#010000');
    expect(rgbToHex(300, -5, 128)).toBe('#ff0080');
    expect(rgbToHex(Number.NaN, 1, 2)).toBe('#000102');
  });
});

describe('nextThemeName', () => {
  it('空列表 → 1;已有 1 → 2', () => {
    expect(nextThemeName([])).toBe('自定义主题 1');
    const one = [mkTheme('a', '自定义主题 1', {})];
    expect(nextThemeName(one)).toBe('自定义主题 2');
  });

  it('删除中间编号后补最小空闲(留 1、3 → 2)', () => {
    const list = [mkTheme('a', '自定义主题 1', {}), mkTheme('c', '自定义主题 3', {})];
    expect(nextThemeName(list)).toBe('自定义主题 2');
  });
});

describe('dedupeName', () => {
  it('不冲突原样返回;冲突追加递增后缀', () => {
    const list = [mkTheme('a', '夜色', {})];
    expect(dedupeName('晨光', list)).toBe('晨光');
    expect(dedupeName('夜色', list)).toBe('夜色 (2)');
    const two = [...list, mkTheme('b', '夜色 (2)', {})];
    expect(dedupeName('夜色', two)).toBe('夜色 (3)');
  });
});
