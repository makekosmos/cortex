<script setup lang="ts">
// Launcher mockup — статическая визуализация LauncherView.vue из shell/src/.
// Структура и стили подобраны под реальный launcher (см. image #10).
// Один rounded window с тенью, без внешнего "desktop bezel".

interface Cmd {
  id: string;
  title: string;
  appName?: string; // справа от title, мутный
  kind: "app" | "command";
  icon: string;
  iconColor: string;
}

const commands: Cmd[] = [
  {
    id: "delphi:open",
    title: "Открыть Delphi",
    kind: "app",
    icon: "★",
    iconColor: "#1d8eff",
  },
  {
    id: "horologion:open",
    title: "Открыть Horologion",
    kind: "app",
    icon: "◐",
    iconColor: "#a855f7",
  },
  {
    id: "dashboard:open",
    title: "Открыть таблицу данных",
    appName: "Kepler",
    kind: "command",
    icon: "▤",
    iconColor: "#22c55e",
  },
  {
    id: "settings:open",
    title: "Открыть настройки",
    appName: "Kepler",
    kind: "command",
    icon: "⚙",
    iconColor: "#737373",
  },
  {
    id: "kepler:check-updates",
    title: "Проверить обновления",
    appName: "Kepler",
    kind: "command",
    icon: "↻",
    iconColor: "#1d8eff",
  },
  {
    id: "delphi:inbox",
    title: "Открыть входящие",
    appName: "Delphi",
    kind: "command",
    icon: "★",
    iconColor: "#1d8eff",
  },
  {
    id: "horologion:pomodoro",
    title: "Помодоро",
    appName: "Horologion",
    kind: "command",
    icon: "◐",
    iconColor: "#a855f7",
  },
  {
    id: "horologion:stopwatch",
    title: "Секундомер",
    appName: "Horologion",
    kind: "command",
    icon: "◐",
    iconColor: "#a855f7",
  },
];

const kindLabel = (k: Cmd["kind"]) => (k === "app" ? "Приложение" : "Команда");
</script>

<template>
  <div class="mockup-frame">
    <div class="launcher" aria-label="Kepler launcher preview">
      <input
        class="search"
        type="text"
        placeholder="Поиск команд: pomo, заметка, открыть delphi…"
        readonly
        tabindex="-1"
      />

      <div class="list">
        <div class="section-label">ВСЕ</div>
        <ul class="results">
          <li v-for="c in commands" :key="c.id" class="result">
            <span class="icon" :style="{ background: c.iconColor }">{{ c.icon }}</span>
            <div class="row-main">
              <span class="title">{{ c.title }}</span>
              <span v-if="c.appName" class="app-name">{{ c.appName }}</span>
            </div>
            <span class="kind-label">{{ kindLabel(c.kind) }}</span>
          </li>
        </ul>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mockup-frame {
  width: 100%;
  max-width: 1024px;
  padding: 0 16px;
  display: flex;
  justify-content: center;
}

.launcher {
  width: 100%;
  background: oklch(0.18 0 0);
  border: 1px solid oklch(0.23 0 0);
  border-radius: 24px;
  padding: 16px 0;
  box-shadow:
    0 1px 0 rgba(255, 255, 255, 0.04) inset,
    0 30px 80px rgba(0, 0, 0, 0.55),
    0 8px 24px rgba(0, 0, 0, 0.4);
  font-family: var(--font-sans);
}

.search {
  width: 100%;
  background: transparent;
  border: none;
  outline: none;
  padding: 12px 32px;
  color: var(--foreground);
  font-size: 16px;
  font-family: inherit;
  font-weight: 400;
  caret-color: var(--accent);
}
.search::placeholder {
  color: oklch(0.5 0 0);
}

.list {
  padding: 8px 0 4px;
}

.section-label {
  font-size: 11px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: oklch(0.45 0 0);
  padding: 12px 32px 8px;
}

.results {
  list-style: none;
  margin: 0;
  padding: 0;
}

.result {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 9px 32px;
}

.icon {
  flex-shrink: 0;
  width: 22px;
  height: 22px;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: white;
  font-size: 12px;
  font-weight: 600;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
}

.row-main {
  display: flex;
  align-items: baseline;
  gap: 10px;
  flex: 1;
  min-width: 0;
}

.row-main .title {
  font-size: 14px;
  color: var(--foreground);
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.row-main .app-name {
  font-size: 13px;
  color: oklch(0.5 0 0);
  white-space: nowrap;
  font-weight: 400;
}

.kind-label {
  flex-shrink: 0;
  font-size: 13px;
  color: oklch(0.5 0 0);
  font-weight: 400;
}

@media (max-width: 720px) {
  .launcher {
    border-radius: 16px;
    padding: 12px 0;
  }
  .search {
    padding: 10px 20px;
    font-size: 14px;
  }
  .section-label {
    padding: 8px 20px 6px;
  }
  .result {
    padding: 8px 20px;
    gap: 10px;
  }
  .row-main .title {
    font-size: 13px;
  }
  .row-main .app-name,
  .kind-label {
    font-size: 11px;
  }
}
</style>
