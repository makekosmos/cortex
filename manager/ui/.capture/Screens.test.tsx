import { open, test } from "nsh/test";
import { Window } from "nsh/desktop";

const pages: Array<{ route: string; label: string }> = [
  { route: "server", label: "Сервер" },
  { route: "appearance", label: "Внешний вид" },
  { route: "about", label: "О приложении" },
  { route: "apps", label: "Приложения" },
  { route: "integrations", label: "Интеграции" },
  { route: "storage", label: "Хранилище" },
  { route: "activity", label: "Активность" },
  { route: "devices", label: "Девайсы" },
  { route: "browser", label: "Браузер" },
  { route: "keys", label: "Ключи" },
  { route: "models", label: "Модели" },
  { route: "settings", label: "Настройки" },
  { route: "developer", label: "Разработчикам" },
];

for (const page of pages) {
  test(`screenshot: ${page.route}`, async () => {
    const app = await open("../App.tsx");
    await app.setViewport(1024, 854);
    Window.setFrame({ origin: { x: -20000, y: -20000 }, size: { width: 1024, height: 854 } });
    if (page.route !== "server") {
      await app.press(page.label);
    }
    await app.screenshot(`${page.route}.png`);
  });
}
