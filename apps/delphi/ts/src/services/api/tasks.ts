import type { Task } from '@/types/task';
import type { TaskDto } from './client';

export function fromTaskDto(dto: TaskDto): Task {
  return {
    ...dto,
    created_at: new Date(dto.created_at),
    updated_at: new Date(dto.updated_at),
  };
}
