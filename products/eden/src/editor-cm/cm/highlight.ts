// Портировано из ZenNotes (MIT, © 2026 Adib Hanna and ZenNotes contributors), адаптировано для Eden.
/**
 * HighlightStyle для Eden markdown-редактора.
 *
 * Маппит lezer-теги на CSS-классы (tok-*). CSS-переменные цветов
 * определяются в cm-editor.css. Используется с syntaxHighlighting()
 * из @codemirror/language.
 *
 * Источник: paperHighlight из ZenNotes EditorPane.tsx.
 */
import { HighlightStyle } from "@codemirror/language";
import { tags as t } from "@lezer/highlight";

export const edenHighlight = HighlightStyle.define([
  // Markdown-level tokens
  { tag: t.heading1, class: "tok-heading1" },
  { tag: t.heading2, class: "tok-heading2" },
  { tag: t.heading3, class: "tok-heading3" },
  { tag: t.heading4, class: "tok-heading4" },
  { tag: t.heading5, class: "tok-heading5" },
  { tag: t.heading6, class: "tok-heading6" },
  { tag: t.emphasis, class: "tok-emphasis" },
  { tag: t.strong, class: "tok-strong" },
  { tag: t.link, class: "tok-link" },
  { tag: t.url, class: "tok-url" },
  { tag: t.monospace, class: "tok-monospace" },
  { tag: t.quote, class: "tok-quote" },
  { tag: t.list, class: "tok-list" },
  { tag: t.meta, class: "tok-meta" },
  // Code-syntax tokens (JS/TS/Python/Go/…) inside fenced blocks.
  { tag: t.keyword, class: "tok-keyword" },
  { tag: t.controlKeyword, class: "tok-keyword" },
  { tag: t.definitionKeyword, class: "tok-keyword" },
  { tag: t.moduleKeyword, class: "tok-keyword" },
  { tag: t.modifier, class: "tok-keyword" },
  { tag: t.operatorKeyword, class: "tok-keyword" },
  { tag: t.string, class: "tok-string" },
  { tag: t.special(t.string), class: "tok-string" },
  { tag: t.regexp, class: "tok-string" },
  { tag: t.character, class: "tok-string" },
  { tag: t.escape, class: "tok-string" },
  { tag: t.comment, class: "tok-comment" },
  { tag: t.lineComment, class: "tok-comment" },
  { tag: t.blockComment, class: "tok-comment" },
  { tag: t.docComment, class: "tok-comment" },
  { tag: t.number, class: "tok-number" },
  { tag: t.integer, class: "tok-number" },
  { tag: t.float, class: "tok-number" },
  { tag: t.bool, class: "tok-atom" },
  { tag: t.atom, class: "tok-atom" },
  { tag: t.null, class: "tok-atom" },
  { tag: t.self, class: "tok-atom" },
  { tag: t.special(t.variableName), class: "tok-atom" },
  { tag: t.operator, class: "tok-operator" },
  { tag: t.logicOperator, class: "tok-operator" },
  { tag: t.arithmeticOperator, class: "tok-operator" },
  { tag: t.bitwiseOperator, class: "tok-operator" },
  { tag: t.compareOperator, class: "tok-operator" },
  { tag: t.updateOperator, class: "tok-operator" },
  { tag: t.definitionOperator, class: "tok-operator" },
  { tag: t.typeOperator, class: "tok-operator" },
  { tag: t.controlOperator, class: "tok-keyword" },
  { tag: t.typeName, class: "tok-type" },
  { tag: t.className, class: "tok-type" },
  { tag: t.namespace, class: "tok-type" },
  { tag: t.standard(t.variableName), class: "tok-type" },
  { tag: t.function(t.variableName), class: "tok-function" },
  { tag: t.function(t.definition(t.variableName)), class: "tok-function" },
  { tag: t.function(t.propertyName), class: "tok-function" },
  { tag: t.definition(t.variableName), class: "tok-variable-def" },
  { tag: t.definition(t.propertyName), class: "tok-variable-def" },
  { tag: t.variableName, class: "tok-variable" },
  { tag: t.propertyName, class: "tok-property" },
  { tag: t.labelName, class: "tok-label" },
  { tag: t.punctuation, class: "tok-punct" },
  { tag: t.separator, class: "tok-punct" },
  { tag: t.bracket, class: "tok-bracket" },
  { tag: t.paren, class: "tok-bracket" },
  { tag: t.brace, class: "tok-bracket" },
  { tag: t.squareBracket, class: "tok-bracket" },
  { tag: t.angleBracket, class: "tok-bracket" },
  { tag: t.tagName, class: "tok-tag" },
  { tag: t.attributeName, class: "tok-attr" },
  { tag: t.attributeValue, class: "tok-string" },
  { tag: t.annotation, class: "tok-meta-code" },
  { tag: t.processingInstruction, class: "tok-meta-code" },
  { tag: t.invalid, class: "tok-invalid" },
]);
