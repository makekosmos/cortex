import { effect, mount, nativeComponent, signal, Text, View } from "nsh";
import { Window as DesktopWindow } from "nsh/desktop";
import { TextInput, cobaltFoundry, settingsMetrics } from "@makekosmos/imago-nsh";
import type { ApplicationTheme, NavGroup } from "@makekosmos/imago-nsh";
import { navigation } from "./navigation";
import { currentRoute as route, go } from "./history";
import { TITLEBAR_HEIGHT, sidebarTopInset } from "./chrome";
import { Sidebar } from "./sidebar";
import { connectHost, dragBinding } from "./host";
import { demoState } from "./state";
import { hotkeyCaptured } from "./state";
import type { ManagerState } from "./state";
import type { CommandArgs } from "./commands";
import { Cancel, Confirm, Modal, note } from "./modal";
import { ServerPage } from "./pages/server";
import { ActivityPage, PreferencesPage } from "./pages/system";
import { StoragePage } from "./pages/storage";
import { KeysPage, ModelsPage } from "./pages/library";
import { PluginPage, PluginsPage } from "./pages/plugins";
import { BrowserPage, DevicesPage, IntegrationsPage } from "./pages/network";

const state = signal<ManagerState>(globalThis.__MUNDUS_MANAGER_INITIAL__ ?? demoState);
// The history owns the route: the sidebar and the Engine's `navigate` push
// onto it, and the header's anchors walk it (`history.ts`).
const search = signal("");

/// The two questions the Manager asks before doing something irreversible:
/// stopping the Engine, and restoring a backup over the live database.
const confirmStop = signal(false);
const confirmInstall = signal(false);
const restoreId = signal("");
/// The pairing code the user types to reach another device.
const connectOpen = signal(false);
const connectCode = signal("");

const Bridge = nativeComponent<
  { route: string },
  {
    state: ManagerState;
    navigate: { route: string };
    hotkey: { accelerator?: string };
    hotkeyState: { capturing: boolean; held: string[] };
  }
>("manager.bridge", {
  props: ["route"],
  events: ["state", "navigate", "hotkey", "hotkeyState"],
});

let session: { command: (name: string, args?: CommandArgs) => void } | undefined;

/** The palette the pages wear. Mundus keeps one: themes belong to the apps the
 * person runs inside it, not to the Manager's own window. */
const colors = (): ApplicationTheme => cobaltFoundry;

function patch(next: Partial<ManagerState>) {
  state.set({ ...state(), ...next });
}

/** Sends an Engine operation. Without a host the call goes nowhere, and the
 * pages keep their demo values so the interface still reads in `nsh dev`. */
function dispatch(name: string, args: CommandArgs = {}) {
  try {
    session?.command(name, args);
  } catch {
    /* `nsh dev` and the screenshot tests have no manager.bridge. */
  }
}

/**
 * What a press means. Irreversible Engine operations ask first, and everything
 * else is a plain call.
 */
function command(name: string, args: CommandArgs = {}) {
  switch (name) {
    case "server.stop":
      confirmStop.set(true);
      return;
    case "updates.install":
      confirmInstall.set(true);
      return;
    case "backups.restore":
      restoreId.set(String(args.id ?? ""));
      return;
    case "connect.open":
      connectOpen.set(true);
      return;
    case "pairing.hide":
      patch({ sync: { ...state().sync, ownCode: "" } });
      return;
    case "route":
      go(String(args.route));
      return;
    default:
      dispatch(name, args);
  }
}

/**
 * A window with no system bar of its own: the first row of the content is the
 * band the caption buttons sit in, and `titleBar()` reports the caption area so
 * the sidebar can inset below it.
 */
function useHiddenChrome() {
  effect(() => DesktopWindow.setTitleBar({ style: "hidden", height: TITLEBAR_HEIGHT }));
}

function renderPage(page: string) {
  const props = { colors: colors(), state: () => state(), onCommand: command };
  switch (page) {
    case "plugins":
      return <PluginsPage {...props} />;
    case "integrations":
      return <IntegrationsPage {...props} />;
    case "storage":
      return <StoragePage {...props} />;
    case "activity":
      return <ActivityPage {...props} />;
    case "devices":
      return <DevicesPage {...props} />;
    case "browser":
      return <BrowserPage {...props} />;
    case "keys":
      return <KeysPage {...props} />;
    case "models":
      return <ModelsPage {...props} />;
    case "settings":
      return <PreferencesPage {...props} />;
    default: {
      // A plugin's own page: `plugin/<id>` is the only route built from data.
      const id = page.startsWith("plugin/") ? page.slice("plugin/".length) : "";
      if (id) return <PluginPage {...props} id={id} />;
      return <ServerPage {...props} />;
    }
  }
}

/**
 * The sidebar: the product's own groups, then the plugins that are installed.
 * A plugin is not a page the Manager was built around — it sits below the
 * machine's settings because it is the person's addition, and it disappears
 * with the plugin.
 */
function sidebarGroups(): NavGroup[] {
  const installed = state().plugins.installed;
  const status = state().engine.status;
  const caption =
    status === "running"
      ? "Работает"
      : status === "starting"
        ? "Запускается"
        : status === "stopping"
          ? "Останавливается"
          : "Не отвечает";
  const groups = navigation.map((group) => ({
    ...group,
    entries: group.entries.map((entry) =>
      entry.route === "server" ? { ...entry, caption } : entry,
    ),
  }));
  if (!installed.length) return groups;
  return [
    ...groups,
    {
      id: "plugins-installed",
      entries: installed.map((plugin) => ({
        route: `plugin/${plugin.id}`,
        label: plugin.name,
        icon: "plug" as const,
        accent: "#15b8a4",
        accentTo: "#1784fc",
      })),
    },
  ];
}

/**
 * The pairing request the Engine is holding, if there is one to answer. The
 * Engine is the one that closes the handshake: the modal leaves only once the
 * poller reports the request gone, so a failed call keeps it on screen.
 */
function pendingPairing() {
  return state().sync.incoming[0];
}

function PairingModal(props: { colors: ApplicationTheme }) {
  const request = pendingPairing();
  if (!request) return null;
  const answer = (name: string) => dispatch(name, { id: request.id });
  const who =
    request.platform && request.appVersion
      ? `${request.platform} · ${request.appVersion}`
      : undefined;
  return (
    <Modal
      title="Запрос на подключение"
      colors={props.colors}
      onDismiss={() => answer("sync.decline")}
      footer={
        <>
          <Cancel colors={props.colors} label="Отклонить" onPress={() => answer("sync.decline")} />
          <Confirm colors={props.colors} label="Принять" onPress={() => answer("sync.accept")} />
        </>
      }
    >
      <Text style={{ fontSize: 13, lineHeight: 19, color: props.colors.foreground }}>
        {`«${request.name}» хочет подключиться к этому компьютеру.`}
      </Text>
      {who ? note(props.colors, who) : null}
      {note(
        props.colors,
        "Устройство получит доступ к задачам, заметкам и истории. Подключайте только то, что узнаёте.",
      )}
    </Modal>
  );
}

function App() {
  useHiddenChrome();
  return (
    <View class="h-full w-full" style={{ backgroundColor: colors().background }}>
      <Bridge
        ref={(node) => {
          session = { command: (name, args) => node.command(name, args) };
          connectHost((name) => node.command(name, {}));
        }}
        route={route()}
        onState={(next: ManagerState) => state.set(next)}
        onNavigate={(event: { route: string }) => go(event.route)}
        onHotkey={(event: { accelerator?: string }) => {
          if (event.accelerator) hotkeyCaptured.set(event.accelerator);
        }}
        onHotkeyState={(event: { capturing: boolean; held: string[] }) => {
          const now = state();
          state.set({ ...now, hotkey: { ...now.hotkey, ...event } });
        }}
      />

      <View class="min-h-0 flex-1 flex-row">
        {/* The sidebar is the window's other titlebar: its search field sits in
            the band, and an empty stretch of it drags — the drag is a
            background of its own under the rows, because a handler on their
            parent would fire for the rows' presses too. */}
        <View
          class="h-full relative overflow-hidden"
          style={{
            width: settingsMetrics.sidebarWidth,
            backgroundColor: colors().shell,
          }}
          {...dragBinding()}
        >
          <View class="absolute inset-0" {...dragBinding()} />
          <Sidebar
            colors={colors()}
            groups={sidebarGroups()}
            current={route()}
            search={search()}
            topInset={sidebarTopInset(settingsMetrics.searchHeight)}
            onSearch={(value) => search.set(value)}
            onNavigate={(next) => go(next)}
          />
        </View>
        {renderPage(route())}
      </View>

      <PairingModal colors={colors()} />

      {confirmStop() ? (
        <Modal
          title="Остановить Mundus?"
          colors={colors()}
          onDismiss={() => confirmStop.set(false)}
          footer={
            <>
              <Cancel colors={colors()} onPress={() => confirmStop.set(false)} />
              <Confirm
                colors={colors()}
                tone="danger"
                label="Остановить"
                onPress={() => {
                  confirmStop.set(false);
                  dispatch("server.stop");
                }}
              />
            </>
          }
        >
          {note(
            colors(),
            "Atlas и диктовка потеряют связь с этим компьютером до следующего запуска. Данные останутся на месте.",
          )}
        </Modal>
      ) : null}

      {confirmInstall() ? (
        <Modal
          title="Установить обновление?"
          colors={colors()}
          onDismiss={() => confirmInstall.set(false)}
          footer={
            <>
              <Cancel colors={colors()} onPress={() => confirmInstall.set(false)} />
              <Confirm
                colors={colors()}
                label="Установить"
                onPress={() => {
                  confirmInstall.set(false);
                  dispatch("updates.install");
                }}
              />
            </>
          }
        >
          {note(
            colors(),
            "Mundus закроется, поставит новую версию и запустится снова. Это займёт около минуты.",
          )}
        </Modal>
      ) : null}

      {restoreId() ? (
        <Modal
          title="Восстановить копию?"
          colors={colors()}
          onDismiss={() => restoreId.set("")}
          footer={
            <>
              <Cancel colors={colors()} onPress={() => restoreId.set("")} />
              <Confirm
                colors={colors()}
                tone="danger"
                label="Восстановить"
                onPress={() => {
                  const id = restoreId();
                  restoreId.set("");
                  dispatch("backups.restore", { id });
                }}
              />
            </>
          }
        >
          {note(
            colors(),
            `База заменится копией ${restoreId()}. Всё, что записано после неё, пропадёт.`,
          )}
        </Modal>
      ) : null}

      {connectOpen() ? (
        <Modal
          title="Подключить устройство"
          colors={colors()}
          onDismiss={() => connectOpen.set(false)}
          footer={
            <>
              <Cancel colors={colors()} onPress={() => connectOpen.set(false)} />
              <Confirm
                colors={colors()}
                label="Подключить"
                onPress={() => {
                  const code = connectCode();
                  connectOpen.set(false);
                  connectCode.set("");
                  dispatch("connect.code", { value: code });
                }}
              />
            </>
          }
        >
          {note(
            colors(),
            "Откройте Mundus на другом устройстве, покажите там код и введите его здесь.",
          )}
          <TextInput
            colors={colors()}
            value={connectCode()}
            placeholder="Код подключения"
            accessibilityLabel="Код подключения"
            onChange={(value) => connectCode.set(value)}
          />
        </Modal>
      ) : null}
    </View>
  );
}

mount(App);
