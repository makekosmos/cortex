// Placeholder vanilla JS — реальный Dashboard будет Vue bundle (Phase 4).
const $time = document.getElementById("time");
function tick() {
  if ($time) $time.textContent = new Date().toLocaleTimeString("ru-RU");
}
tick();
setInterval(tick, 1000);
