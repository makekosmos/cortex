import { effect, mount, nativeComponent, signal, Text, View } from "nsh";
import { Window as DesktopWindow } from "nsh/desktop";
import { platform } from "os";
import {
  SettingsSidebar,
  TextInput,
  cobaltFoundry,
  darkThemes,
  lightThemes,
  settingsMetrics,
} from "@makekosmos/imago-nsh";
import type { ApplicationTheme } from "@makekosmos/imago-nsh";
import { navigation } from "./navigation";
import { demoState } from "./state";
import type { ManagerState } from "./state";
import type { CommandArgs } from "./commands";
import { Cancel, Confirm, Modal, note } from "./modal";
import { ServerPage } from "./pages/server";
import { AppearancePage } from "./pages/appearance";
import { AboutPage, ActivityPage, PreferencesPage, StoragePage } from "./pages/system";
import { AppsPage, KeysPage, ModelsPage } from "./pages/library";
import { BrowserPage, DevicesPage, IntegrationsPage } from "./pages/network";

const state = signal<ManagerState>(globalThis.__MUNDUS_MANAGER_INITIAL__ ?? demoState);
const route = signal<string>(globalThis.__MUNDUS_MANAGER_ROUTE__ ?? "server");
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
  { state: ManagerState; navigate: { route: string } }
>("manager.bridge", {
  props: ["route"],
  events: ["state", "navigate"],
});

let session: { command: (name: string, args?: CommandArgs) => void } | undefined;

/** The palette the pages wear: whichever pair of themes the Engine says is on.
 * A light mode must not paint a dark card set, or the pages would say «светлая
 * тема» over dark cards. */
const colors = (): ApplicationTheme => {
  const { mode, light, dark } = state().appearance;
  const themes = mode === "light" ? lightThemes : darkThemes;
  return (
    themes.find((theme) => theme.key === (mode === "light" ? light : dark))?.colors ?? cobaltFoundry
  );
};

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
 * What a press means. Irreversible Engine operations ask first, appearance is
 * a device-wide setting on the Engine, and everything else is a plain call.
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
    case "appearance.theme":
      dispatch("appearance.set", {
        patch: args.mode === "light" ? { light_theme: args.key } : { dark_theme: args.key },
      });
      patch({
        appearance: {
          ...state().appearance,
          [args.mode === "light" ? "light" : "dark"]: String(args.key),
        },
      });
      return;
    case "pairing.hide":
      patch({ sync: { ...state().sync, ownCode: "" } });
      return;
    case "appearance.followApps":
      dispatch("appearance.set", { patch: { follow_apps: Boolean(args.value) } });
      patch({ appearance: { ...state().appearance, followApps: Boolean(args.value) } });
      return;
    case "route":
      route.set(String(args.route));
      return;
    default:
      dispatch(name, args);
  }
}

/**
 * A window with no system bar of its own: the page heading is the top row, and
 * `titleBar()` reports the caption area so the sidebar can inset below it.
 */
const TITLEBAR_HEIGHT = 36;
/// Windows draws a caption button at this width (`46 × 3` is the area
/// `titleBar()` reserves on the right when the system's own are there).
const CAPTION_BUTTON_WIDTH = 46;

function useHiddenChrome() {
  effect(() => DesktopWindow.setTitleBar({ style: "hidden", height: TITLEBAR_HEIGHT }));
}

/**
 * Whether the strip on the right of the band belongs to the window's caption
 * buttons. On Windows it always does: an embedded host's buttons are drawn by
 * `manager/src/chrome.rs`, and a host that owns its window (`nsh run`, `nsh
 * dev`) draws them itself and reports the strip in `titleBar()`. Either way the
 * band must keep that strip out of its drag area, or a press on a button starts
 * a window move and the button never sees it.
 */
function reservesCaptions() {
  const host = platform();
  return host === "win32" || host === "windows";
}

function TitleBar(props: { colors: ApplicationTheme }) {
  // macOS keeps AppKit's traffic lights in this band's left corner.
  const macos = platform() === "darwin";
  return (
    <View class="w-full shrink-0 flex-row" style={{ height: TITLEBAR_HEIGHT }}>
      {/* Each half carries the colour of what stands under it, so the band reads
          as the top of the sidebar and the top of the page, not as a third bar. */}
      <View
        desktop={{ windowDrag: true }}
        class="h-full flex-row items-center"
        style={{
          width: settingsMetrics.sidebarWidth,
          backgroundColor: props.colors.shell,
          paddingLeft: macos ? 80 : 14,
        }}
      >
        <Text style={{ fontSize: 12, fontWeight: "500", color: props.colors.muted }}>Mundus</Text>
      </View>
      <View
        desktop={{ windowDrag: true }}
        class="h-full flex-1"
        style={{ backgroundColor: props.colors.background }}
      />
      {reservesCaptions() ? (
        <View
          style={{
            width: CAPTION_BUTTON_WIDTH * 3,
            height: TITLEBAR_HEIGHT,
            backgroundColor: props.colors.background,
          }}
        />
      ) : null}
    </View>
  );
}

function renderPage(page: string) {
  const props = { colors: colors(), state: () => state(), onCommand: command };
  switch (page) {
    case "about":
      return <AboutPage {...props} />;
    case "appearance":
      return <AppearancePage {...props} />;
    case "apps":
      return <AppsPage {...props} />;
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
    default:
      return <ServerPage {...props} />;
  }
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
        }}
        route={route()}
        onState={(next: ManagerState) => state.set(next)}
        onNavigate={(event: { route: string }) => route.set(event.route)}
      />
      <TitleBar colors={colors()} />

      <View class="min-h-0 flex-1 flex-row">
        {/* The sidebar is the window's other titlebar: an empty stretch of it
            drags, and a row that takes a press stays a row — which is why the
            drag area is a background of its own under the rows, not a handler on
            their parent (a handler there would fire for the rows' presses too). */}
        <View class="h-full flex-col relative" desktop={{ windowDrag: true }}>
          <View class="absolute inset-0" desktop={{ windowDrag: true }} />
          <SettingsSidebar
            colors={colors()}
            groups={navigation}
            current={route()}
            search={search()}
            onSearch={(value) => search.set(value)}
            onNavigate={(next) => route.set(next)}
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
