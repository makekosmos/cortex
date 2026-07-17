export interface CodingSubmission {
  id: string;
  problemTitle: string;
  problemSlug: string;
  status: string;
  accepted: boolean;
  language: string;
  runtime: string;
  memory: string;
  submittedAt: string;
  url: string;
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
