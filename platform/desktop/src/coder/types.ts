export type CoderPlatform = "leetcode" | "codewars";

export interface CodingSubmission {
  id: string;
  source: CoderPlatform;
  username: string;
  problemTitle: string;
  problemSlug: string;
  problemNumber: string;
  status: string;
  accepted: boolean;
  language: string;
  languages: string[];
  runtime: string;
  memory: string;
  submittedAt: string;
  url: string;
}

export interface CodewarsProfileStats {
  username: string;
  honor: number;
  leaderboardPosition: number;
  rankName: string;
  rankScore: number;
}

export interface CoderDifficultyStats {
  easy: number;
  easyTotal: number;
  medium: number;
  mediumTotal: number;
  hard: number;
  hardTotal: number;
  total: number;
  available: number;
}

export interface CoderSummary {
  submissions: number;
  accepted: number;
  solved: number;
  acceptanceRate: number;
  currentStreak: number;
  longestStreak: number;
}

export interface CoderBreakdownItem {
  label: string;
  count: number;
  share: number;
}

export interface CoderActivityDay {
  date: string;
  count: number;
  level: number;
}
