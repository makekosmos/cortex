import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupAction,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInput,
  SidebarInset,
  SidebarMenu,
  SidebarMenuAction,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSkeleton,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
  SidebarProvider,
  SidebarRail,
  SidebarSeparator,
  SidebarTrigger,
  useSidebar,
} from "@/components/ui/sidebar";

function SidebarStateProbe() {
  const { state, open, toggleSidebar, setOpen } = useSidebar();
  return (
    <div>
      <div data-testid="state">{state}</div>
      <div data-testid="open">{open ? "open" : "closed"}</div>
      <button type="button" onClick={toggleSidebar}>
        toggle
      </button>
      <button type="button" onClick={() => setOpen(true)}>
        open
      </button>
      <button type="button" onClick={() => setOpen(false)}>
        close
      </button>
      <button type="button" onClick={() => setOpen((prev: boolean) => !prev)}>
        flip
      </button>
    </div>
  );
}

describe("ui/sidebar", () => {
  beforeEach(() => {
    // Desktop by default.
    Object.defineProperty(window, "innerWidth", {
      value: 1024,
      writable: true,
    });
    document.cookie = "";
  });

  it("throws if useSidebar is used outside provider", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    expect(() => render(<SidebarStateProbe />)).toThrow(
      "useSidebar must be used within a SidebarProvider.",
    );
    spy.mockRestore();
  });

  it("toggles sidebar open state and writes cookie", async () => {
    render(
      <SidebarProvider defaultOpen>
        <SidebarStateProbe />
        <Sidebar collapsible="icon">
          <SidebarHeader>Header</SidebarHeader>
          <SidebarContent>Content</SidebarContent>
          <SidebarFooter>Footer</SidebarFooter>
        </Sidebar>
        <SidebarInset>Inset</SidebarInset>
        <SidebarTrigger />
        <SidebarRail />
      </SidebarProvider>,
    );

    expect(screen.getByTestId("state")).toHaveTextContent("expanded");
    const trigger = document.querySelector('[data-slot="sidebar-trigger"]');
    if (!trigger) throw new Error("missing sidebar-trigger");
    await userEvent.click(trigger);
    expect(screen.getByTestId("state")).toHaveTextContent("collapsed");
    expect(document.cookie).toContain("sidebar_state=");

    // Cover setOpen functional updater path explicitly.
    await userEvent.click(screen.getByRole("button", { name: "flip" }));
    expect(screen.getByTestId("state")).toHaveTextContent("expanded");
    // Cover setOpen boolean path explicitly.
    await userEvent.click(screen.getByRole("button", { name: "close" }));
    expect(screen.getByTestId("state")).toHaveTextContent("collapsed");
    await userEvent.click(screen.getByRole("button", { name: "open" }));
    expect(screen.getByTestId("state")).toHaveTextContent("expanded");

    // Keyboard shortcut (Ctrl+B).
    window.dispatchEvent(
      new KeyboardEvent("keydown", { key: "b", ctrlKey: true }),
    );
    await waitFor(() =>
      expect(screen.getByTestId("state")).toHaveTextContent("expanded"),
    );

    // Keyboard shortcut no-op (missing modifier / wrong key) for branch coverage.
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "b" }));
    window.dispatchEvent(
      new KeyboardEvent("keydown", { key: "x", ctrlKey: true }),
    );
  });

  it("supports controlled open state via open/onOpenChange", async () => {
    const onOpenChange = vi.fn();

    render(
      <SidebarProvider open={false} onOpenChange={onOpenChange}>
        <SidebarTrigger />
        <Sidebar collapsible="icon">
          <SidebarHeader>Header</SidebarHeader>
          <SidebarContent>Content</SidebarContent>
        </Sidebar>
      </SidebarProvider>,
    );

    await userEvent.click(
      screen.getByRole("button", { name: /переключить боковую панель/i }),
    );
    expect(onOpenChange).toHaveBeenCalledWith(true);
  });

  it("renders collapsible=none variant", () => {
    render(
      <SidebarProvider>
        <Sidebar collapsible="none">Body</Sidebar>
      </SidebarProvider>,
    );

    expect(screen.getByText("Body")).toBeInTheDocument();
  });

  it("renders mobile sheet sidebar when isMobile", async () => {
    Object.defineProperty(window, "innerWidth", { value: 0, writable: true });

    render(
      <SidebarProvider defaultOpen>
        <SidebarTrigger />
        <Sidebar side="left">
          <SidebarHeader>Header</SidebarHeader>
          <SidebarContent>Content</SidebarContent>
        </Sidebar>
      </SidebarProvider>,
    );

    // Toggle should open mobile sheet.
    await userEvent.click(
      screen.getByRole("button", { name: /переключить боковую панель/i }),
    );

    await waitFor(() => {
      expect(
        document.querySelector('[data-slot="sidebar"][data-mobile="true"]'),
      ).toBeTruthy();
    });
  });

  it("renders menu primitives including tooltips and skeleton", async () => {
    const onMenuClick = vi.fn();

    render(
      <SidebarProvider defaultOpen={false}>
        <Sidebar collapsible="icon">
          <SidebarMenu>
            <SidebarMenuItem>
              <SidebarMenuButton
                tooltip="Tip"
                onClick={onMenuClick}
                aria-label="menu-button"
              >
                Menu
              </SidebarMenuButton>
              <SidebarMenuButton
                tooltip={{ children: "TipObj" }}
                aria-label="menu-button-tip-obj"
              >
                MenuObj
              </SidebarMenuButton>
              <SidebarMenuButton aria-label="menu-button-no-tip">
                NoTip
              </SidebarMenuButton>
              <SidebarMenuAction aria-label="menu-action">A</SidebarMenuAction>
              <SidebarMenuBadge>9</SidebarMenuBadge>
            </SidebarMenuItem>
            <SidebarMenuSkeleton showIcon />
            <SidebarMenuSub>
              <SidebarMenuSubItem>
                <SidebarMenuSubButton href="#x">Sub</SidebarMenuSubButton>
              </SidebarMenuSubItem>
            </SidebarMenuSub>
          </SidebarMenu>
          <SidebarSeparator />
          <SidebarGroup>
            <SidebarGroupLabel>Group</SidebarGroupLabel>
            <SidebarGroupAction aria-label="group-action">+</SidebarGroupAction>
            <SidebarGroupContent>GroupContent</SidebarGroupContent>
          </SidebarGroup>
          <SidebarInput aria-label="sidebar-input" />
        </Sidebar>
      </SidebarProvider>,
    );

    await userEvent.click(screen.getByLabelText("menu-button"));
    expect(onMenuClick).toHaveBeenCalled();
    expect(screen.getByLabelText("menu-button-tip-obj")).toBeInTheDocument();
    expect(screen.getByText("9")).toBeInTheDocument();
    expect(screen.getByText("GroupContent")).toBeInTheDocument();
    expect(screen.getByLabelText("sidebar-input")).toBeInTheDocument();
  });

  it("covers floating/inset variants and right side layout branches", () => {
    render(
      <SidebarProvider defaultOpen={false}>
        <Sidebar side="right" variant="floating" collapsible="icon">
          <SidebarContent>FloatingRight</SidebarContent>
        </Sidebar>
        <Sidebar side="left" variant="inset" collapsible="icon">
          <SidebarContent>InsetLeft</SidebarContent>
        </Sidebar>
      </SidebarProvider>,
    );

    expect(screen.getByText("FloatingRight")).toBeInTheDocument();
    expect(screen.getByText("InsetLeft")).toBeInTheDocument();
  });

  it("supports SidebarMenuAction asChild + showOnHover and SidebarMenuSubButton variants", () => {
    render(
      <SidebarProvider defaultOpen={false}>
        <Sidebar collapsible="icon">
          <SidebarMenu>
            <SidebarMenuItem>
              <SidebarMenuButton aria-label="peer" data-size="sm">
                Peer
              </SidebarMenuButton>
              <SidebarMenuAction asChild showOnHover aria-label="action">
                <button type="button" data-testid="action-child" />
              </SidebarMenuAction>
            </SidebarMenuItem>
            <SidebarMenuSub>
              <SidebarMenuSubItem>
                <SidebarMenuSubButton size="sm" href="#sm">
                  Small
                </SidebarMenuSubButton>
              </SidebarMenuSubItem>
              <SidebarMenuSubItem>
                <SidebarMenuSubButton asChild size="md" isActive>
                  <a data-testid="sub-child" href="#md">
                    Sub
                  </a>
                </SidebarMenuSubButton>
              </SidebarMenuSubItem>
            </SidebarMenuSub>
          </SidebarMenu>
        </Sidebar>
      </SidebarProvider>,
    );

    const action = screen.getByTestId("action-child");
    expect(action).toHaveAttribute("data-sidebar", "menu-action");

    const subChild = screen.getByTestId("sub-child");
    expect(subChild).toHaveAttribute("data-sidebar", "menu-sub-button");
    expect(subChild).toHaveAttribute("data-active", "true");
    expect(subChild).toHaveAttribute("data-size", "md");
  });

  it("supports asChild for group label/action and menu button (including string tooltip)", () => {
    render(
      <SidebarProvider defaultOpen={false}>
        <Sidebar collapsible="icon">
          <SidebarMenu>
            <SidebarMenuItem>
              <SidebarMenuButton asChild tooltip="Tip">
                <button type="button" data-testid="menu-button-child">
                  Menu
                </button>
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
          <SidebarGroup>
            <SidebarGroupLabel asChild>
              <div data-testid="group-label-child">Group</div>
            </SidebarGroupLabel>
            <SidebarGroupAction asChild>
              <button type="button" data-testid="group-action-child" />
            </SidebarGroupAction>
          </SidebarGroup>
        </Sidebar>
      </SidebarProvider>,
    );

    expect(screen.getByTestId("menu-button-child")).toHaveAttribute(
      "data-sidebar",
      "menu-button",
    );
    expect(screen.getByTestId("group-label-child")).toHaveAttribute(
      "data-sidebar",
      "group-label",
    );
    expect(screen.getByTestId("group-action-child")).toHaveAttribute(
      "data-sidebar",
      "group-action",
    );
  });
});
