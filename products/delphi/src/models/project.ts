import { type Project, ProjectStatus } from "@/types/task";

const uuid = () => crypto.randomUUID();

export type CreateProjectParams = {
  title: string;

  notes?: string | null;

  status?: ProjectStatus;

  areaId?: string | null;

  colorTag?: string | null;

  billable?: boolean;

  price?: number | null;
};

export function createProject(params: CreateProjectParams): Project {
  return {
    id: uuid(),

    title: params.title,

    notes: params.notes ?? null,

    status: params.status ?? ProjectStatus.Active,

    scheduledDate: null,

    deadline: null,

    sortOrder: 0,

    colorTag: params.colorTag ?? null,

    createdAt: new Date().toISOString(),

    areaId: params.areaId ?? null,

    billable: params.billable ?? false,

    price: params.price ?? null,
  };
}
