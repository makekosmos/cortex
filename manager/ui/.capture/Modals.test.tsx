import { open, test } from "nsh/test";
import { Window } from "nsh/desktop";

/// The two questions the Manager asks before something irreversible, captured
/// as the user sees them. (The pairing request needs a live Engine snapshot, so
/// its card is captured from a state that holds a request: see `report`.)

test("modal: stop engine", async () => {
  const app = await open("../App.tsx");
  await app.setViewport(1024, 854);
  Window.setFrame({ origin: { x: -20000, y: -20000 }, size: { width: 1024, height: 854 } });
  await app.press("Остановить");
  await app.screenshot("modal-stop.png");
});

test("modal: connect device", async () => {
  const app = await open("../App.tsx");
  await app.setViewport(1024, 854);
  Window.setFrame({ origin: { x: -20000, y: -20000 }, size: { width: 1024, height: 854 } });
  await app.press("Девайсы");
  await app.press("Подключить");
  await app.screenshot("modal-connect.png");
});
