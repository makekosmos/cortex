import type { Meta, StoryObj } from "@storybook/vue3-vite";
import SyncNodeRow from "./SyncNodeRow.vue";

const meta: Meta<typeof SyncNodeRow> = {
  title: "Settings/SyncNodeRow",
  component: SyncNodeRow,
  args: {
    deviceKind: "desktop",
    name: "MacBook Pro",
    lastSeenLabel: "Сейчас подключено",
    status: "online",
    statusLabel: "Онлайн",
  },
};

export default meta;
type Story = StoryObj<typeof SyncNodeRow>;

export const DesktopOnline: Story = {};
export const LaptopOffline: Story = {
  args: {
    deviceKind: "laptop",
    name: "Work Laptop",
    lastSeenLabel: "Последнее подключение: 17 июн. 2026 г., 12:34",
    status: "offline",
    statusLabel: "Оффлайн",
  },
};
export const PhoneOnline: Story = {
  args: {
    deviceKind: "phone",
    name: "iPhone",
    lastSeenLabel: "Сейчас подключено",
    status: "online",
    statusLabel: "Онлайн",
  },
};
export const Disconnecting: Story = {
  args: {
    deviceKind: "unknown",
    name: "Unknown Device",
    lastSeenLabel: "Сейчас подключено",
    status: "connecting",
    statusLabel: "Отключение...",
    disconnecting: true,
  },
};
