export type SettingsSectionId =
  | "all"
  | "appearance"
  | "system"
  | "backup"
  | "compression"
  | "sqoba"
  | "rawg";

export type SettingsSectionIcon =
  | "monitor"
  | "power"
  | "shield"
  | "hardDrive"
  | "sparkles"
  | "key";

export interface SettingsSectionItem {
  id: SettingsSectionId;
  label: string;
  icon: SettingsSectionIcon;
}

export type InlineFeedbackTone = "info" | "success" | "error";

export interface InlineFeedback {
  tone: InlineFeedbackTone;
  text: string;
}
