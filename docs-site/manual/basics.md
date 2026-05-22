---
title: Быстрый старт
description: От установки до первой заметки за 5 шагов.
---

<div class="eyebrow">Начало</div>

# Быстрый старт

Первый раз с Kosmos? Этот гайд проведёт тебя через установку, поиск
приложений, ведение заметок, задач и запуск фокусной сессии.

<ol class="quickstart-steps">
  <li>
    <div class="step-number">1</div>
    <div class="step-body">
      <h3 class="step-title">Скачай Kepler</h3>
      <p>Лаунчер Kosmos. Поставь, назначь горячую клавишу — и получишь свой shortcut ко всему.</p>
      <p>
        <a class="step-button" href="https://github.com/yoso-industries/kepler-releases/releases/latest" target="_blank" rel="noopener">
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" width="14" height="14"><path d="M8 1.75v9m0 0 3.25-3.25M8 10.75 4.75 7.5"/><path d="M3 13.25h10"/></svg>
          Скачать Kepler
        </a>
      </p>
      <p class="step-subtitle">Системные требования</p>
      <ul class="step-list">
        <li><span class="badge">Beta</span> Windows 10 (build 1809+) или Windows 11 (x64)</li>
        <li><span class="badge dim">В планах</span> macOS</li>
        <li><span class="badge dim">В планах</span> Android, iOS</li>
      </ul>
    </div>
  </li>
  <li>
    <div class="step-number">2</div>
    <div class="step-body">
      <h3 class="step-title">Открой поисковик</h3>
      <p>Нажми <kbd>Alt</kbd>+<kbd>Space</kbd> — появится окно поисковика 720×460. Это твоя точка входа во всё: запуск приложений, команды расширений, заметки, задачи.</p>
      <p>Если хочется другую клавишу — Настройки → Общие → Горячая клавиша.</p>
    </div>
  </li>
  <li>
    <div class="step-number">3</div>
    <div class="step-body">
      <h3 class="step-title">Запускай приложения</h3>
      <p>Все установленные программы и UWP-приложения сразу в списке. Начни печатать имя — список сужается. <kbd>Enter</kbd> — запуск.</p>
      <p>Подробнее — <a href="./core-features/app-launcher">Запуск приложений</a>.</p>
    </div>
  </li>
  <li>
    <div class="step-number">4</div>
    <div class="step-body">
      <h3 class="step-title">Создай заметку или задачу</h3>
      <p>Расширения Kosmos регистрируют команды в том же поисковике. Напечатай <strong>заметку</strong> — увидишь команду «Создать заметку» от Eden. <strong>задачу</strong> — задачу создаст Delphi.</p>
      <p>Подробнее — <a href="./extensions/eden">Eden</a> и <a href="./extensions/delphi">Delphi</a>.</p>
    </div>
  </li>
  <li>
    <div class="step-number">5</div>
    <div class="step-body">
      <h3 class="step-title">Запусти фокусную сессию</h3>
      <p>Напечатай <strong>помодоро 25</strong> — Horologion стартанёт 25-минутный таймер. Появится плавающий виджет поверх всех окон, который тикает даже если ты закроешь Horologion.</p>
      <p>Подробнее — <a href="./extensions/horologion">Horologion</a>.</p>
    </div>
  </li>
</ol>

<div class="next-block">
  <a href="./platforms" class="next-link">
    <span class="next-label">Дальше</span>
    <span class="next-title">Платформы <span aria-hidden="true">→</span></span>
  </a>
</div>

<style scoped>
.eyebrow {
  font-size: 13px;
  color: var(--kosmos-muted-fg);
  margin-bottom: -8px;
  margin-top: 4px;
}

h1 {
  font-size: 40px;
  letter-spacing: -0.6px;
  font-weight: 800;
  margin-bottom: 14px;
}

.quickstart-steps {
  list-style: none;
  padding: 0;
  margin: 32px 0 0;
  counter-reset: step;
}

.quickstart-steps > li {
  position: relative;
  display: grid;
  grid-template-columns: 32px 1fr;
  gap: 16px;
  padding-bottom: 28px;
}

/* Соединительная вертикальная линия между circles */
.quickstart-steps > li:not(:last-child)::before {
  content: "";
  position: absolute;
  left: 15px;
  top: 32px;
  bottom: 0;
  width: 1px;
  background: var(--kosmos-border);
}

.step-number {
  width: 28px;
  height: 28px;
  border-radius: 999px;
  background: var(--vp-c-bg-soft);
  border: 1px solid var(--kosmos-border);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 13px;
  font-weight: 600;
  color: var(--kosmos-muted-fg);
  position: relative;
  z-index: 1;
}

.step-body {
  padding-top: 2px;
}

.step-title {
  margin: 0 0 8px;
  font-size: 18px;
  font-weight: 600;
  letter-spacing: -0.2px;
  color: var(--kosmos-fg);
  border: none;
  padding: 0;
}

.step-body p {
  margin: 0 0 10px;
  line-height: 1.55;
  color: var(--kosmos-muted-fg);
  font-size: 14.5px;
}

.step-body a {
  color: var(--vp-c-brand-1);
  text-decoration: none;
}

.step-body a:hover {
  text-decoration: underline;
}

.step-subtitle {
  font-weight: 600;
  color: var(--kosmos-fg) !important;
  margin-top: 14px !important;
  margin-bottom: 6px !important;
  font-size: 14px !important;
}

.step-list {
  list-style: none;
  padding: 0;
  margin: 0 0 0 4px;
}

.step-list > li {
  position: relative;
  padding: 3px 0 3px 16px;
  font-size: 14px;
  color: var(--kosmos-muted-fg);
}

.step-list > li::before {
  content: "○";
  position: absolute;
  left: 0;
  color: var(--kosmos-muted-fg);
  font-size: 10px;
  top: 6px;
}

.step-button {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 8px 14px;
  border: 1px solid var(--kosmos-border);
  background: var(--vp-c-bg-soft);
  border-radius: 8px;
  color: var(--kosmos-fg) !important;
  font-size: 13.5px;
  font-weight: 500;
  text-decoration: none !important;
  transition: background 120ms ease, border-color 120ms ease;
}

.step-button:hover {
  background: var(--vp-c-brand-soft);
  border-color: var(--vp-c-brand-1);
}

.badge {
  display: inline-block;
  padding: 1px 7px;
  margin-right: 6px;
  border-radius: 4px;
  font-size: 10.5px;
  font-weight: 600;
  letter-spacing: 0.02em;
  background: var(--vp-c-brand-soft);
  color: var(--vp-c-brand-1);
  text-transform: lowercase;
  vertical-align: 1px;
}

.badge.dim {
  background: transparent;
  color: var(--kosmos-muted-fg);
  border: 1px solid var(--kosmos-border);
}

kbd {
  display: inline-block;
  padding: 1px 6px;
  font-family: var(--kosmos-font-mono);
  font-size: 12px;
  color: var(--kosmos-muted-fg);
  background: var(--vp-c-bg-soft);
  border: 1px solid var(--kosmos-border);
  border-radius: 4px;
  vertical-align: 1px;
}

.next-block {
  margin-top: 24px;
  padding-top: 18px;
  border-top: 1px solid var(--kosmos-border);
}

.next-link {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  text-align: right;
  text-decoration: none !important;
}

.next-label {
  font-size: 13px;
  color: var(--kosmos-muted-fg);
  margin-bottom: 2px;
}

.next-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--vp-c-brand-1);
}

.next-link:hover .next-title {
  text-decoration: underline;
}
</style>
