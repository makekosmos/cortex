import { Loader2, Mic, Plus, Square } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { startLiveVoiceTasks } from '@/services/gemini/liveVoiceTasks';
import useTask from '@/store/tasks';
import type { Task } from '@/types/task';

type VoiceState = 'idle' | 'connecting' | 'recording' | 'stopping';
type LiveVoiceHandle = Awaited<ReturnType<typeof startLiveVoiceTasks>>;

function resolveVoiceError(error: unknown) {
  if (error instanceof Error) return error.message;
  return 'Voice task creation failed.';
}

export default function TaskCreateButton() {
  const addTask = useTask((s) => s.addTask);
  const editTask = useTask((s) => s.editTask);
  const removeTask = useTask((s) => s.removeTask);

  const [voiceState, setVoiceState] = useState<VoiceState>('idle');
  const [voiceStatus, setVoiceStatus] = useState<string | null>(null);
  const [transcriptPreview, setTranscriptPreview] = useState('');

  const liveVoiceRef = useRef<LiveVoiceHandle | null>(null);
  const operationQueueRef = useRef<Promise<unknown>>(Promise.resolve());
  const sessionTasksRef = useRef<Task[]>([]);
  const createdCountRef = useRef(0);

  useEffect(() => {
    return () => {
      if (liveVoiceRef.current) {
        void liveVoiceRef.current.stop();
        liveVoiceRef.current = null;
      }
    };
  }, []);

  const runInQueue = <T,>(task: () => Promise<T>) => {
    const next = operationQueueRef.current.then(task, task);
    operationQueueRef.current = next.catch(() => undefined);
    return next;
  };

  const incrementCreatedCount = (delta: number) => {
    const nextValue = Math.max(0, createdCountRef.current + delta);
    createdCountRef.current = nextValue;
  };

  const startVoiceMode = async () => {
    if (voiceState !== 'idle') return;

    setVoiceState('connecting');
    setVoiceStatus('Connecting to Gemini Live...');
    setTranscriptPreview('');
    createdCountRef.current = 0;
    sessionTasksRef.current = [];

    try {
      const liveVoice = await startLiveVoiceTasks({
        onCreateTask: (title) =>
          runInQueue(async () => {
            const createdTask = await addTask(title);
            sessionTasksRef.current.push(createdTask);
            incrementCreatedCount(1);
            return true;
          }),
        onUpdateLastTask: (title) =>
          runInQueue(async () => {
            const stack = sessionTasksRef.current;
            if (stack.length === 0) return false;

            const lastTask = stack[stack.length - 1];
            const updatedTask: Task = {
              ...lastTask,
              title,
            };
            await editTask(updatedTask);
            stack[stack.length - 1] = updatedTask;
            return true;
          }),
        onDeleteLastTask: () =>
          runInQueue(async () => {
            const stack = sessionTasksRef.current;
            if (stack.length === 0) return false;

            const lastTask = stack.pop();
            if (!lastTask) return false;
            await removeTask(lastTask);
            incrementCreatedCount(-1);
            return true;
          }),
        onTranscript: (transcript) => {
          setTranscriptPreview(transcript.slice(-240));
        },
        onError: (message) => {
          liveVoiceRef.current = null;
          setVoiceStatus(message);
          setVoiceState('idle');
        },
      });

      liveVoiceRef.current = liveVoice;
      setVoiceState('recording');
      setVoiceStatus('Listening... tasks are created while you speak.');
    } catch (error) {
      setVoiceState('idle');
      setVoiceStatus(resolveVoiceError(error));
    }
  };

  const stopVoiceMode = async () => {
    if (!liveVoiceRef.current) {
      setVoiceState('idle');
      return;
    }

    setVoiceState('stopping');
    setVoiceStatus('Stopping voice session...');

    try {
      await liveVoiceRef.current.stop();
      await operationQueueRef.current;

      const count = createdCountRef.current;
      setVoiceStatus(
        count > 0
          ? `Voice session ended. Created tasks: ${count}.`
          : 'Voice session ended. No new tasks.',
      );
    } catch (error) {
      setVoiceStatus(resolveVoiceError(error));
    } finally {
      liveVoiceRef.current = null;
      setVoiceState('idle');
    }
  };

  const handleVoiceClick = () => {
    if (voiceState === 'idle') {
      void startVoiceMode();
      return;
    }
    if (voiceState === 'recording') {
      void stopVoiceMode();
    }
  };

  const voiceButtonLabel =
    voiceState === 'recording'
      ? 'Stop live voice session'
      : voiceState === 'connecting'
        ? 'Connecting live voice session'
        : voiceState === 'stopping'
          ? 'Stopping live voice session'
          : 'Start live voice session';

  const isVoiceBusy = voiceState === 'connecting' || voiceState === 'stopping';
  const hasTranscript = transcriptPreview.length > 0 && voiceState === 'recording';

  return (
    <div className="max-w-threadcontentwidth w-full px-2">
      <div className="flex w-full items-center gap-2">
        <button
          type="button"
          className="group flex h-15 flex-1 cursor-pointer items-center justify-center gap-2.5 rounded-xl border border-(--border) bg-(--secondary) text-(--secondary-foreground) hover:text-(--secondary-foreground)"
          onClick={() => {
            void addTask('');
          }}
        >
          <span className="font-mono group-hover:hidden">Create task</span>
          <Plus size={22} />
        </button>

        <button
          type="button"
          aria-label={voiceButtonLabel}
          title={voiceButtonLabel}
          disabled={isVoiceBusy}
          className={`flex h-15 w-15 items-center justify-center rounded-xl border border-(--border) bg-(--secondary) text-(--secondary-foreground) hover:text-(--secondary-foreground) disabled:cursor-not-allowed disabled:opacity-70 ${voiceState === 'recording' ? 'animate-pulse border-rose-500 text-rose-500' : ''}`}
          onClick={handleVoiceClick}
        >
          {isVoiceBusy ? (
            <Loader2 size={20} className="animate-spin" />
          ) : voiceState === 'recording' ? (
            <Square size={20} />
          ) : (
            <Mic size={20} />
          )}
        </button>
      </div>

      {voiceStatus ? (
        <div className="mt-2 text-center text-xs text-(--muted-foreground)">
          {voiceStatus}
        </div>
      ) : null}
      {hasTranscript ? (
        <div className="mt-1 text-center text-[11px] text-(--muted-foreground)">
          {transcriptPreview}
        </div>
      ) : null}
    </div>
  );
}
