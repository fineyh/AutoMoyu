// 界面语言：文案写成 t("中文", "English")，两种说法放在一起，改一处不会漏另一处。
// 用哪种由后端定（Status.lang：设置里选的，或跟随 Windows 显示语言）；拿到之前先看浏览器语言。

import type { Lang } from "./types";

let lang: Lang = navigator.language.toLowerCase().startsWith("zh") ? "zh" : "en";

export function setLang(l: Lang) {
  lang = l;
  document.documentElement.lang = l === "en" ? "en" : "zh-CN";
}

export const getLang = () => lang;
export const isEn = () => lang === "en";

export function t(zh: string, en: string) {
  return lang === "en" ? en : zh;
}

/** 英文复数：plural(3, "fish") / plural(1, "session", "sessions")。 */
export function plural(n: number, one: string, many = one) {
  return `${n} ${n === 1 ? one : many}`;
}
