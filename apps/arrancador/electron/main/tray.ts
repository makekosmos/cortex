import { Menu, type NativeImage, Tray } from "electron";

export interface AppTrayOptions {
  icon: NativeImage;
  onToggle: () => void;
  onShow: () => void;
  onHide: () => void;
  onQuit: () => void | Promise<void>;
}

export function createAppTray(options: AppTrayOptions): Tray {
  const tray = new Tray(options.icon);
  tray.setToolTip("Arrancador");

  const menu = Menu.buildFromTemplate([
    {
      label: "\u041f\u043e\u043a\u0430\u0437\u0430\u0442\u044c \u0438\u043b\u0438 \u0441\u043a\u0440\u044b\u0442\u044c \u043e\u043a\u043d\u043e",
      click: () => options.onToggle(),
    },
    {
      type: "separator",
    },
    {
      label: "\u041f\u043e\u043a\u0430\u0437\u0430\u0442\u044c \u043e\u043a\u043d\u043e",
      click: () => options.onShow(),
    },
    {
      label: "\u0421\u043a\u0440\u044b\u0442\u044c \u0432 \u0442\u0440\u0435\u0439",
      click: () => options.onHide(),
    },
    {
      type: "separator",
    },
    {
      label: "\u0412\u044b\u0445\u043e\u0434",
      click: () => {
        void options.onQuit();
      },
    },
  ]);

  tray.setContextMenu(menu);
  tray.on("click", () => options.onToggle());

  return tray;
}
