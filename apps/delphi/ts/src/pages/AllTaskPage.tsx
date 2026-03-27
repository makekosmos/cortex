import TaskCreateButton from '@/components/TaskCreateButton';
import TaskList from '@/components/taskList';

export default function AllTaskPage() {
  return (
    <div className="flex w-full min-w-0 flex-col">
      <div className="flex justify-center border-b border-(--border) p-5 align-middle">
        <h1 className="font-mono select-none">Active tasks</h1>
      </div>
      <TaskList sortType="notCompleted" />

      <div className="flex justify-center px-4 py-5">
        <TaskCreateButton />
      </div>
    </div>
  );
}
