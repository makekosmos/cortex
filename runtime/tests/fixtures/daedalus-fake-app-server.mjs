import fs from "node:fs";
import readline from "node:readline";

const lines = readline.createInterface({ input: process.stdin, crlfDelay: Infinity });
const send = (value) => process.stdout.write(`${JSON.stringify(value)}\n`);
let approvalId = 900;
let initialized = false;
let activeTurnId = null;
let expectedPolicy = null;

lines.on("line", (line) => {
  const message = JSON.parse(line);
  if (!message.method && message.id === approvalId) {
    send({ method: "item/completed", params: { item: { id: "file-change", type: "fileChange" } } });
    send({ method: "turn/completed", params: { turn: { id: "fake-turn", status: "completed" } } });
    return;
  }
  if (message.method === "initialized") {
    initialized = true;
    return;
  }
  if (message.method !== "initialize" && !initialized) {
    send({ id: message.id, error: { code: -32002, message: "initialized notification required" } });
    return;
  }
  switch (message.method) {
    case "initialize":
      send({ id: message.id, result: { userAgent: "daedalus-fake" } });
      break;
    case "model/list":
      send({
        id: message.id,
        result: {
          data: [
            {
              id: "fake-model",
              displayName: "Fake model",
              description: "E2E",
              model: "fake-model",
              isDefault: true,
              hidden: false,
              defaultReasoningEffort: "medium",
              supportedReasoningEfforts: [],
            },
          ],
          nextCursor: null,
        },
      });
      break;
    case "thread/start":
    case "thread/resume":
      expectedPolicy = {
        approvalPolicy: message.params.approvalPolicy,
        approvalsReviewer: message.params.approvalsReviewer,
        sandboxType:
          message.params.sandbox === "danger-full-access" ? "dangerFullAccess" : "workspaceWrite",
      };
      send({
        id: message.id,
        result: {
          thread: { id: "fake-thread", turns: [] },
          model: "fake-model",
          modelProvider: "fake",
          cwd: process.cwd(),
          approvalPolicy: "on-request",
          approvalsReviewer: "user",
          sandbox: { type: "workspaceWrite" },
        },
      });
      break;
    case "turn/start":
      if (
        !message.params?.approvalPolicy ||
        !message.params?.sandboxPolicy?.type ||
        !message.params?.approvalsReviewer ||
        message.params.approvalPolicy !== expectedPolicy?.approvalPolicy ||
        message.params.approvalsReviewer !== expectedPolicy?.approvalsReviewer ||
        message.params.sandboxPolicy.type !== expectedPolicy?.sandboxType
      ) {
        send({ id: message.id, error: { code: -32602, message: "missing turn policy" } });
        return;
      }
      fs.writeFileSync("daedalus-e2e.txt", "изменено fake app-server\n", "utf8");
      activeTurnId = "fake-turn";
      send({
        id: message.id,
        result: { turn: { id: "fake-turn", status: "inProgress", items: [] } },
      });
      send({
        method: "turn/started",
        params: { threadId: "fake-thread", turn: { id: "fake-turn", status: "inProgress" } },
      });
      send({
        method: "turn/plan/updated",
        params: { text: "Создать файл и запросить подтверждение" },
      });
      send({ method: "item/agentMessage/delta", params: { delta: "Файл подготовлен. " } });
      send({ method: "item/agentMessage/delta", params: { delta: "Нужно подтверждение." } });
      if (message.params.input?.[0]?.text?.includes("вопрос")) {
        send({
          id: approvalId,
          method: "item/tool/requestUserInput",
          params: {
            threadId: "fake-thread",
            turnId: "fake-turn",
            itemId: "question",
            questions: [
              {
                id: "choice",
                header: "Вариант",
                question: "Как продолжить?",
                options: [
                  { label: "Быстро", description: "Минимальный вариант" },
                  { label: "Тщательно", description: "Полная проверка" },
                ],
              },
            ],
          },
        });
        break;
      }
      send({
        method: "turn/diff/updated",
        params: { threadId: "fake-thread", turnId: "fake-turn" },
      });
      send({
        id: approvalId,
        method: "item/fileChange/requestApproval",
        params: {
          threadId: "fake-thread",
          turnId: "fake-turn",
          itemId: "file-change",
          reason: "Добавить daedalus-e2e.txt",
          grantRoot: process.cwd(),
        },
      });
      break;
    case "turn/steer":
      if (message.params?.expectedTurnId !== activeTurnId) {
        send({ id: message.id, error: { code: -32602, message: "unexpected active turn" } });
        return;
      }
      send({ id: message.id, result: { turnId: activeTurnId } });
      send({
        method: "item/agentMessage/delta",
        params: { itemId: "steer", delta: "Уточнение принято." },
      });
      break;
    case "turn/interrupt":
      send({ id: message.id, result: {} });
      send({
        method: "turn/completed",
        params: { turn: { id: "fake-turn", status: "interrupted" } },
      });
      activeTurnId = null;
      break;
    default:
      send({ id: message.id, result: {} });
  }
});
