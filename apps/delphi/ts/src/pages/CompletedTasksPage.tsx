import TaskList from '@/components/taskList';

export default function CompletedTasksPage() {
  return (
    <div className="flex w-full flex-col">
      <div className="flex justify-center border-b border-(--border) p-5 align-middle">
        <h1 className="font-mono select-none">Completed tasks</h1>
      </div>

      <TaskList sortType="completed" />
    </div>
  );
}
