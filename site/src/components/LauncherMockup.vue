<script setup lang="ts">
// Launcher mockup — статическая визуализация LauncherView.vue из shell/src/
// которая всегда отражает реальный UI приложения. При изменении launcher'а
// тут обновляются те же классы + структура.

interface Cmd {
  id: string;
  title: string;
  subtitle?: string; // appName на правой части
  kind: "app" | "command";
  icon: string; // emoji or short label
  iconColor: string;
}

const commands: Cmd[] = [
  {
    id: "delphi:open",
    title: "Открыть Delphi",
    kind: "app",
    icon: "★",
    iconColor: "#1e90ff",
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
    subtitle: "Kepler",
    kind: "command",
    icon: "▤",
    iconColor: "#22c55e",
  },
  {
    id: "settings:open",
    title: "Открыть настройки",
    subtitle: "Kepler",
    kind: "command",
    icon: "⚙",
    iconColor: "#737373",
  },
  {
    id: "kepler:check-updates",
    title: "Проверить обновления",
    subtitle: "Kepler",
    kind: "command",
    icon: "↻",
    iconColor: "#0ea5e9",
  },
  {
    id: "delphi:inbox",
    title: "Открыть входящие",
    subtitle: "Delphi",
    kind: "command",
    icon: "★",
    iconColor: "#1e90ff",
  },
  {
    id: "horologion:pomodoro",
    title: "Помодоро",
    subtitle: "Horologion",
    kind: "command",
    icon: "◐",
    iconColor: "#a855f7",
  },
  {
    id: "horologion:stopwatch",
    title: "Секундомер",
    subtitle: "Horologion",
    kind: "command",
    icon: "◐",
    iconColor: "#a855f7",
  },
];

const kindLabel = (k: Cmd["kind"]) => (k === "app" ? "Приложение" : "Команда");
</script>

<template>
  <div class="mockup-frame">
    <!-- "Desktop frame" — внешняя рамка вокруг launcher'а -->
    <div class="desktop-bezel">
      <!-- Real launcher rendered inside -->
      <div class="launcher" aria-label="Kepler launcher preview">
        <input
          class="search"
          type="text"
          placeholder="Поиск команд: pomo, заметка, открыть delphi…"
          readonly
          tabindex="-1"
        />

        <div class="list">
          <div class="section-label">Все</div>
          <ul class="results">
            <li v-for="c in commands" :key="c.id" class="result">
              <span class="icon" :style="{ background: c.iconColor }">{{ c.icon }}</span>
              <div class="row-main">
                <span class="title">{{ c.title }}</span>
                <span v-if="c.subtitle" class="subtitle">{{ c.subtitle }}</span>
              </div>
              <span class="kind-label">{{ kindLabel(c.kind) }}</span>
            </li>
          </ul>
        </div>
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

/* External "desktop bezel" — тёмная рамка вокруг launcher'а, как macbook frame */
.desktop-bezel {
  width: 100%;
  background: linear-gradient(180deg, #1a1a1a 0%, #0e0e0e 100%);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  padding: 40px 60px 80px;
  box-shadow:
    0 1px 0 rgba(255, 255, 255, 0.04) inset,
    0 30px 80px rgba(0, 0, 0, 0.5),
    0 8px 24px rgba(0, 0, 0, 0.3);
}

/* Launcher window — фон/border соответствует реальному shell launcher'у */
.launcher {
  width: 100%;
  max-width: 720px;
  margin: 0 auto;
  background: #1a1a1a;
  border: 1px solid #2a2a2a;
  border-radius: var(--radius);
  padding: 12px 0;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
  font-family: var(--font-sans);
}

.search {
  width: 100%;
  background: transparent;
  border: none;
  outline: none;
  padding: 12px 20px;
  color: var(--foreground);
  font-size: 15px;
  font-family: inherit;
  caret-color: var(--accent-bright);
}
.search::placeholder {
  color: var(--muted-2);
}

.list {
  padding: 8px 0 4px;
}

.section-label {
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--muted-2);
  padding: 8px 20px 4px;
}

.results {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0;
}

.result {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 20px;
  cursor: default;
  border-radius: 8px;
  margin: 0 8px;
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

.row-main .subtitle {
  font-size: 13px;
  color: var(--muted-2);
  white-space: nowrap;
  font-weight: 400;
}

.kind-label {
  flex-shrink: 0;
  font-size: 13px;
  color: var(--muted-2);
  font-weight: 400;
}

@media (max-width: 720px) {
  .desktop-bezel {
    padding: 20px 16px 40px;
    border-radius: var(--radius);
  }
  .launcher {
    max-width: 100%;
  }
  .result {
    margin: 0 4px;
    padding: 8px 12px;
    gap: 10px;
  }
  .row-main .subtitle,
  .kind-label {
    font-size: 11px;
  }
  .row-main .title {
    font-size: 13px;
  }
}
</style>
