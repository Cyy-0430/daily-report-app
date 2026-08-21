import type { ThemeColors } from './theme';

/**
 * 主题定制的工作状态,模块级 $state(应用生命周期单例)。
 * 切换设置 tab / 离开设置页再回来,draft(未保存的调色现场)与 preview 均保留
 * (与报告页状态保留同构,见 state-management.md)。
 * 只放数据;编排(选择/预览/保存/重命名/删除)在 ThemeTab 组件。
 * 注意:.svelte.ts 模块只能导出 const 的 $state 对象,变更一律改属性。
 */
export const theme = $state({
  /** 编辑现场;null = 尚未进入编辑(ThemeTab 首次渲染时以激活主题初始化)。 */
  draft: null as null | { baseId: string; colors: ThemeColors },
  /** 预览中的颜色集;null = 未预览。仅内存,不落盘,关闭应用自动恢复激活主题。 */
  preview: null as null | ThemeColors,
});
