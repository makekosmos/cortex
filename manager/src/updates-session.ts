// Session-scoped update check flag for the Manager window: the automatic
// check fires once per app session; later runs come only from the explicit
// "Проверить обновления" button or a new app start.
let sessionChecked = false;

export function claimUpdatesSessionCheck(): boolean {
  if (sessionChecked) return false;
  sessionChecked = true;
  return true;
}
