export interface VimMotionItem {
  keys: string;
  title: string;
  description: string;
}

export interface VimMotionGroup {
  id: string;
  title: string;
  items: VimMotionItem[];
}

export const VIM_MOTION_GROUPS: VimMotionGroup[] = [
  {
    id: "modes",
    title: "Режимы",
    items: [
      { keys: "i", title: "Ввод", description: "Войти во ввод перед курсором." },
      { keys: "I", title: "Ввод в начале строки", description: "Войти во ввод в начале строки." },
      { keys: "a", title: "Ввод после курсора", description: "Войти во ввод после курсора." },
      { keys: "A", title: "Ввод в конце строки", description: "Войти во ввод в конце строки." },
      {
        keys: "o",
        title: "Новая строка ниже",
        description: "Создать строку ниже и перейти во ввод.",
      },
      {
        keys: "O",
        title: "Новая строка выше",
        description: "Создать строку выше и перейти во ввод.",
      },
      { keys: "Esc", title: "Обычный режим", description: "Вернуться в режим команд." },
      { keys: "v", title: "Посимвольное выделение", description: "Выделение по символам." },
      { keys: "V", title: "Построчное выделение", description: "Выделение целыми строками." },
      {
        keys: "Ctrl+v",
        title: "Блочное выделение",
        description: "Выделение прямоугольным блоком.",
      },
    ],
  },
  {
    id: "motion",
    title: "Навигация",
    items: [
      { keys: "h j k l", title: "Движение", description: "Влево, вниз, вверх, вправо." },
      { keys: "w / W", title: "Следующее слово", description: "К началу следующего слова." },
      {
        keys: "e / E",
        title: "Конец слова",
        description: "К концу текущего или следующего слова.",
      },
      { keys: "b / B", title: "Предыдущее слово", description: "К началу предыдущего слова." },
      { keys: "ge / gE", title: "Назад к концу слова", description: "К концу предыдущего слова." },
      { keys: "0", title: "Начало строки", description: "К первой колонке строки." },
      { keys: "^", title: "Первый текст", description: "К первому непробельному символу строки." },
      { keys: "$", title: "Конец строки", description: "К концу строки." },
      { keys: "gg", title: "Начало документа", description: "Перейти к первой строке." },
      { keys: "G", title: "Конец документа", description: "Перейти к последней строке." },
      { keys: "{ / }", title: "Абзацы", description: "К предыдущему или следующему абзацу." },
      { keys: "%", title: "Парная скобка", description: "Перейти к парной скобке." },
      {
        keys: "Ctrl+d / Ctrl+u",
        title: "Полстраницы",
        description: "Прокрутить вниз или вверх на полстраницы.",
      },
      {
        keys: "Ctrl+f / Ctrl+b",
        title: "Страница",
        description: "Прокрутить вниз или вверх на страницу.",
      },
    ],
  },
  {
    id: "editing",
    title: "Редактирование",
    items: [
      {
        keys: "x / X",
        title: "Удалить символ",
        description: "Удалить символ под курсором или перед ним.",
      },
      { keys: "dd", title: "Удалить строку", description: "Удалить текущую строку." },
      { keys: "D", title: "Удалить до конца", description: "Удалить от курсора до конца строки." },
      { keys: "cc", title: "Заменить строку", description: "Удалить строку и перейти во ввод." },
      {
        keys: "C",
        title: "Заменить до конца",
        description: "Удалить до конца строки и перейти во ввод.",
      },
      { keys: "yy", title: "Скопировать строку", description: "Скопировать текущую строку." },
      { keys: "p / P", title: "Вставить", description: "Вставить после или до курсора." },
      { keys: "u", title: "Отмена", description: "Отменить последнее действие." },
      { keys: "Ctrl+r", title: "Повтор отмены", description: "Повторить отменённое действие." },
      { keys: ".", title: "Повтор", description: "Повторить последнее изменение." },
      { keys: "r", title: "Заменить символ", description: "Заменить символ под курсором." },
      {
        keys: "~",
        title: "Сменить регистр",
        description: "Сменить регистр символа или выделения.",
      },
    ],
  },
  {
    id: "search",
    title: "Поиск",
    items: [
      { keys: "/", title: "Поиск вперёд", description: "Открыть поиск по документу." },
      { keys: "?", title: "Поиск назад", description: "Искать вверх по документу." },
      {
        keys: "n / N",
        title: "Следующее совпадение",
        description: "Перейти к следующему или предыдущему совпадению.",
      },
      {
        keys: "* / #",
        title: "Слово под курсором",
        description: "Искать текущее слово вперёд или назад.",
      },
    ],
  },
  {
    id: "eden",
    title: "Команды Eden",
    items: [
      { keys: ":w", title: "Сохранить", description: "Сразу сохранить текущую заметку." },
      { keys: ":q", title: "Закрыть заметку", description: "Закрыть текущую заметку в Eden." },
      { keys: ":wq", title: "Сохранить и закрыть", description: "Сохранить заметку и закрыть её." },
      {
        keys: ":zen",
        title: "Включить focus mode",
        description: "Открыть редактор без лишнего chrome.",
      },
      {
        keys: ":zen on",
        title: "Включить focus mode",
        description: "Открыть редактор без лишнего chrome.",
      },
    ],
  },
];
