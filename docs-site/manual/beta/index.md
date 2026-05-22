---
title: Бета мануал
description: Частые вопросы о текущем состоянии Kosmos.
---

# Бета мануал <span class="beta-pill">β</span>

Kosmos сейчас в активной разработке. Ниже — частые вопросы про текущее
состояние: что работает, что нет, чего ждать.

<details class="faq-item">
<summary>Что такое «бета»?</summary>

Kosmos — личный проект. Это значит:

- Релизы выходят, когда я считаю что фича готова, а не по календарю.
- Часть фич уже стабильна и используется мной ежедневно, часть — на
  раннем этапе и может меняться.
- Я ставлю себе апдейты раньше, чем кто-либо ещё. Если что-то ломается
  — обычно я знаю об этом первым.

</details>

<details class="faq-item">
<summary>Почему Kosmos только для Windows?</summary>

Я работаю на Windows. Это первая платформа потому, что я могу постоянно
тестировать продукт на своей машине. macOS / Android / iOS — в планах,
порядок описан на [странице «Платформы»](../platforms).

</details>

<details class="faq-item">
<summary>Где хранятся мои данные?</summary>

Локально, в `%APPDATA%\Kosmos\ark.db` — это SQLite-база.
**Local-first**: данные принадлежат тебе, не уходят на сервер без
твоего ведома.

LAN-синхронизация между твоими устройствами — в разработке.

</details>

<details class="faq-item">
<summary>Что-то сломалось — куда писать?</summary>

- GitHub Issues: [yoso-industries/kepler](https://github.com/yoso-industries)
- Email: kazajackyyy@gmail.com

Приложи к репорту:

1. Версию Kepler (Settings → О программе)
2. Что делал перед поломкой
3. Логи (если есть) из `%APPDATA%\Kosmos\crashes\`

</details>

<details class="faq-item">
<summary>Я могу потерять данные?</summary>

Технически — да, как и в любом софте. Что Kosmos делает чтобы этого не было:

- **Автобэкап БД** раз в 24 часа в `%APPDATA%\Kosmos\backups\`,
  хранится последние 7 копий.
- **Integrity check** при старте — если БД повредилась, ты узнаешь
  сразу, а не через неделю.
- **Crash reporter** — паники Rust-бэкенда пишутся в файл,
  не теряются молча.

Тем не менее, **резервные копии важных заметок и задач делай сам**, особенно
если данных накопилось много.

</details>

<details class="faq-item">
<summary>Когда стабильный релиз?</summary>

Не знаю. Это личный проект, графика нет. Стабильным считаю момент, когда:

- ARK schema перестанет меняться (сейчас additive-only, но всё ещё в движении)
- LAN-sync пройдёт долгое использование без потерь данных
- Все 4 встроенных расширения дойдут до фич, которыми я сам пользуюсь ежедневно

</details>

<details class="faq-item">
<summary>Я хочу помочь / законтрибьютить</summary>

Пиши на email или открой issue с предложением. Я открыт к фидбеку,
но при этом продукт делаю под себя — не каждое предложение войдёт в
roadmap.

</details>

<details class="faq-item">
<summary>Где changelog?</summary>

[Новости](/whats-new/) — релиз-ноты по версиям Kepler и каждого расширения.

</details>

<style scoped>
.beta-pill {
  display: inline-block;
  padding: 3px 10px;
  margin-left: 6px;
  font-size: 0.42em;
  font-weight: 600;
  letter-spacing: 0.02em;
  color: var(--vp-c-brand-1);
  background: var(--vp-c-brand-soft);
  border-radius: 999px;
  vertical-align: middle;
  text-transform: lowercase;
  font-family: var(--kosmos-font-sans);
}
</style>

<style>
.faq-item {
  margin: 8px 0;
  padding: 0;
  border: 1px solid var(--kosmos-border);
  border-radius: 10px;
  background: transparent;
  transition: border-color 120ms ease;
}

.faq-item[open] {
  border-color: var(--vp-c-brand-1);
}

.faq-item > summary {
  cursor: pointer;
  list-style: none;
  padding: 14px 18px;
  font-size: 15px;
  font-weight: 600;
  color: var(--kosmos-fg);
  user-select: none;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.faq-item > summary::-webkit-details-marker {
  display: none;
}

.faq-item > summary::after {
  content: "+";
  flex-shrink: 0;
  width: 20px;
  height: 20px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 18px;
  font-weight: 400;
  color: var(--kosmos-muted-fg);
  transition: transform 160ms ease, color 120ms ease;
}

.faq-item[open] > summary::after {
  content: "−";
  color: var(--vp-c-brand-1);
}

.faq-item > summary:hover {
  color: var(--vp-c-brand-1);
}

.faq-item > *:not(summary) {
  padding: 0 18px;
}

.faq-item > *:not(summary):first-of-type {
  padding-top: 4px;
}

.faq-item > *:not(summary):last-child {
  padding-bottom: 16px;
}
</style>
