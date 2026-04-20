import { ChevronLeft, ChevronRight } from "lucide-react";
import { cn } from "@/lib/utils";

type TitlebarHistoryControlsProps = {
  backDisabled?: boolean;
  forwardDisabled?: boolean;
  backTitle?: string;
  forwardTitle?: string;
  onBack: () => void;
  onForward: () => void;
};

export function TitlebarHistoryControls({
  backDisabled = false,
  forwardDisabled = false,
  backTitle = "Назад",
  forwardTitle = "Вперёд",
  onBack,
  onForward,
}: TitlebarHistoryControlsProps) {
  return (
    <div className="kepler-titlebar-history-controls">
      <button
        type="button"
        className={cn(
          "kepler-titlebar-history-controls__button",
          backDisabled && "kepler-titlebar-history-controls__button--disabled",
        )}
        disabled={backDisabled}
        title={backTitle}
        aria-label={backTitle}
        data-testid="titlebar-history-back"
        onClick={() => {
          if (!backDisabled) {
            onBack();
          }
        }}
      >
        <ChevronLeft className="h-[14px] w-[14px]" />
      </button>

      <button
        type="button"
        className={cn(
          "kepler-titlebar-history-controls__button",
          forwardDisabled &&
            "kepler-titlebar-history-controls__button--disabled",
        )}
        disabled={forwardDisabled}
        title={forwardTitle}
        aria-label={forwardTitle}
        data-testid="titlebar-history-forward"
        onClick={() => {
          if (!forwardDisabled) {
            onForward();
          }
        }}
      >
        <ChevronRight className="h-[14px] w-[14px]" />
      </button>
    </div>
  );
}
