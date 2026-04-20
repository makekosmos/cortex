import * as ScrollAreaPrimitive from "@radix-ui/react-scroll-area";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { Button, buttonVariants } from "@/components/ui/button";
import {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuPortal,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import { ScrollArea, ScrollBar } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import {
  Sheet,
  SheetClose,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";

describe("UI primitives", () => {
  it("renders Button (default and asChild) and exposes buttonVariants", async () => {
    expect(buttonVariants({ variant: "secondary", size: "sm" })).toContain(
      "bg-secondary",
    );

    render(
      <div>
        <Button>Click</Button>
        <Button asChild>
          <a href="#x">Link</a>
        </Button>
      </div>,
    );

    expect(screen.getByRole("button", { name: "Click" })).toHaveAttribute(
      "data-slot",
      "button",
    );
    expect(screen.getByRole("link", { name: "Link" })).toHaveAttribute(
      "data-slot",
      "button",
    );

    await userEvent.click(screen.getByRole("button", { name: "Click" }));
  });

  it("renders Card building blocks", () => {
    render(
      <Card>
        <CardHeader>
          <CardTitle>Title</CardTitle>
          <CardDescription>Desc</CardDescription>
          <CardAction>Action</CardAction>
        </CardHeader>
        <CardContent>Content</CardContent>
        <CardFooter>Footer</CardFooter>
      </Card>,
    );

    expect(screen.getByText("Title")).toHaveAttribute(
      "data-slot",
      "card-title",
    );
    expect(screen.getByText("Desc")).toHaveAttribute(
      "data-slot",
      "card-description",
    );
    expect(screen.getByText("Action")).toHaveAttribute(
      "data-slot",
      "card-action",
    );
    expect(screen.getByText("Content")).toHaveAttribute(
      "data-slot",
      "card-content",
    );
    expect(screen.getByText("Footer")).toHaveAttribute(
      "data-slot",
      "card-footer",
    );
  });

  it("renders Input, Switch, Separator, Skeleton, Progress, ScrollArea", async () => {
    const enabledOnClick = vi.fn();

    function SwitchHarness() {
      const [checked, setChecked] = useState(true);
      return (
        <>
          <Switch
            aria-label="switch"
            checked={checked}
            onCheckedChange={setChecked}
            onClick={enabledOnClick}
          />
          <Switch
            aria-label="switch-disabled"
            checked={true}
            disabled
            onCheckedChange={vi.fn()}
          />
        </>
      );
    }

    const { container } = render(
      <div>
        <Input aria-label="input" defaultValue="hello" />
        <SwitchHarness />
        <Separator />
        <Separator orientation="vertical" />
        <Skeleton />
        <Progress value={50} />
        <Progress />
        <ScrollArea>
          <div style={{ height: 200 }}>scroll</div>
        </ScrollArea>
        {/* Horizontal scrollbar branch */}
        <ScrollAreaPrimitive.Root>
          <ScrollAreaPrimitive.Viewport />
          <ScrollBar orientation="horizontal" />
        </ScrollAreaPrimitive.Root>
      </div>,
    );

    expect(screen.getByLabelText("input")).toHaveValue("hello");
    expect(screen.getByLabelText("switch")).toHaveAttribute("role", "switch");
    expect(container.querySelectorAll('[data-slot="separator"]').length).toBe(
      2,
    );

    // Progress styles: 50% -> translateX(-50%); undefined -> -100%
    const indicators = Array.from(
      container.querySelectorAll('[data-slot="progress-indicator"]'),
    );
    expect(indicators.length).toBe(2);
    expect(indicators[0]).toHaveStyle({ transform: "translateX(-50%)" });
    expect(indicators[1]).toHaveStyle({ transform: "translateX(-100%)" });

    await userEvent.click(screen.getByLabelText("switch"));
    expect(enabledOnClick).toHaveBeenCalled();

    const disabledSwitch = screen.getByLabelText("switch-disabled");
    expect(disabledSwitch).toBeDisabled();
  });

  it("renders Tooltip content", async () => {
    render(
      <TooltipProvider delayDuration={0}>
        <Tooltip>
          <TooltipTrigger asChild>
            <button type="button">Target</button>
          </TooltipTrigger>
          <TooltipContent>Tip</TooltipContent>
        </Tooltip>
      </TooltipProvider>,
    );

    await userEvent.hover(screen.getByRole("button", { name: "Target" }));
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Tip");
  });

  it("renders Sheet open/close primitives", async () => {
    render(
      <div>
        <Sheet defaultOpen>
          <SheetTrigger asChild>
            <button type="button">Open</button>
          </SheetTrigger>
          <SheetContent>
            <SheetHeader>
              <SheetTitle>Sheet</SheetTitle>
              <SheetDescription>Desc</SheetDescription>
            </SheetHeader>
            <div>Body</div>
            <SheetFooter>
              <SheetClose asChild>
                <button type="button">Close</button>
              </SheetClose>
            </SheetFooter>
          </SheetContent>
        </Sheet>

        {/* Cover side variants */}
        <Sheet defaultOpen>
          <SheetContent side="top">
            <div>Top</div>
          </SheetContent>
        </Sheet>
        <Sheet defaultOpen>
          <SheetContent side="bottom">
            <div>Bottom</div>
          </SheetContent>
        </Sheet>
      </div>,
    );

    expect(screen.getByText("Body")).toBeInTheDocument();
    expect(screen.getByText("Top")).toBeInTheDocument();
    expect(screen.getByText("Bottom")).toBeInTheDocument();
    const close = screen.getAllByRole("button", { name: "Close", hidden: true })[0];
    if (!close) throw new Error("missing sheet-close");
    // Radix may apply pointer-events during transitions; `fireEvent` avoids
    // user-event's strict pointer-events checks while still executing handlers.
    fireEvent.click(close);
    await waitFor(() =>
      expect(screen.queryByText("Body")).not.toBeInTheDocument(),
    );
  });

  it("renders DropdownMenu primitives (items, checkbox, radio, sub)", async () => {
    const onSelect = vi.fn();

    render(
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button type="button">Trigger</button>
        </DropdownMenuTrigger>
        <DropdownMenuPortal />
        <DropdownMenuContent>
          <DropdownMenuLabel>Label</DropdownMenuLabel>
          <DropdownMenuGroup>
            <DropdownMenuItem onSelect={onSelect}>Item</DropdownMenuItem>
            <DropdownMenuCheckboxItem checked>
              Check
              <DropdownMenuShortcut>⌘K</DropdownMenuShortcut>
            </DropdownMenuCheckboxItem>
            <DropdownMenuRadioGroup value="a">
              <DropdownMenuRadioItem value="a">A</DropdownMenuRadioItem>
              <DropdownMenuRadioItem value="b">B</DropdownMenuRadioItem>
            </DropdownMenuRadioGroup>
            <DropdownMenuSeparator />
            <DropdownMenuSub>
              <DropdownMenuSubTrigger>More</DropdownMenuSubTrigger>
              <DropdownMenuSubContent>
                <DropdownMenuItem>SubItem</DropdownMenuItem>
              </DropdownMenuSubContent>
            </DropdownMenuSub>
          </DropdownMenuGroup>
        </DropdownMenuContent>
      </DropdownMenu>,
    );

    await userEvent.click(screen.getByRole("button", { name: "Trigger" }));
    expect(await screen.findByText("Label")).toBeInTheDocument();
    expect(screen.getByText("Check")).toBeInTheDocument();
    expect(screen.getByText("⌘K")).toBeInTheDocument();
    expect(screen.getByText("A")).toBeInTheDocument();
    expect(screen.getByText("B")).toBeInTheDocument();
    expect(screen.getByText("More")).toBeInTheDocument();
    await userEvent.hover(screen.getByText("More"));
    expect(await screen.findByText("SubItem")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("menuitem", { name: "Item" }));
    expect(onSelect).toHaveBeenCalled();
  });
});
