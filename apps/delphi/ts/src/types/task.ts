export type Task = {
  id: string;
  title: string;
  description?: string | null;
  completed: boolean;
  priority?: number;
  due_date?: string | null;
  list_id?: string | null;
  user_id?: string;
  created_at: Date;
  updated_at?: Date;
};
