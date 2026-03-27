import { useEffect, useRef, useState } from 'react';
import useTask from '@/store/tasks';
import type { Task } from '@/types/task';

export default function TaskItem({
  task,
  onToggle,
}: {
  task: Task;
  onToggle: (t: Task) => Promise<void>;
}) {
  const [isEditMode, setIsEditMode] = useState(false);
  const [isMutatedTask, setIsMutatedTask] = useState<Task>({ ...task });
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const editTask = useTask((s) => s.editTask);

  useEffect(() => {
    const textarea = textareaRef.current;
    if (!textarea) return;
    textarea.style.height = 'auto';
    textarea.style.height = `${textarea.scrollHeight}px`;
  });

  const enableEditMode = () => {
    if (isEditMode) return;
    setIsMutatedTask({ ...task });
    setIsEditMode(true);
  };

  const disableEditMode = () => {
    setIsEditMode(false);
    void editTask(isMutatedTask);
  };

  return (
    <li
      className={`flex w-full gap-5 rounded-2xl border-2 border-(--background) p-4 hover:border-(--secondary) hover:bg-(--secondary) ${task.completed ? 'opacity-70' : ''} ${
        isEditMode
          ? 'hover:bg-secondary border-2 border-(--border) bg-(--secondary) hover:border-(--border)!'
          : ''
      }`}
      onDoubleClick={enableEditMode}
    >
      <button
        type="button"
        onClick={() => {
          void onToggle(task);
        }}
        className="self-start"
      >
        <div className="h-5.5 w-5.5 rounded-md border border-(--ring) bg-(--secondary) hover:bg-(--ring)" />
      </button>

      <div className="flex w-full min-w-0 flex-col gap-2.5">
        {!isEditMode ? (
          <div className="flex-1 truncate">{task.title}</div>
        ) : (
          <textarea
            ref={textareaRef}
            name={task.id}
            autoFocus
            className="w-full resize-none overflow-hidden text-amber-600"
            rows={1}
            onBlur={disableEditMode}
            onKeyDown={(event) => {
              if (event.key === 'Enter' || event.key === 'Escape') {
                event.preventDefault();
                disableEditMode();
              }
            }}
            value={isMutatedTask.title}
            onChange={(event) => {
              setIsMutatedTask((prev) => ({
                ...prev,
                title: event.target.value,
              }));
            }}
          />
        )}

        <div className="flex gap-2.5 font-mono font-bold text-(--muted-foreground)">
          <div>
            {task.created_at.toLocaleDateString('en-US', {
              day: '2-digit',
              month: '2-digit',
              year: 'numeric',
            })}
          </div>
          <div>|</div>
          <div>Synced task</div>
        </div>
      </div>
    </li>
  );
}
