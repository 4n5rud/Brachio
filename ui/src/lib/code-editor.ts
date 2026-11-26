// The code editor of the file tab. Only the file tab's view imports this module, and it does so
// behind a dynamic import, so CodeMirror stays out of the app's start-up chunk.
import { closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import {
  HighlightStyle,
  StreamLanguage,
  bracketMatching,
  foldGutter,
  foldKeymap,
  indentOnInput,
  indentUnit,
  syntaxHighlighting,
} from "@codemirror/language";
import { highlightSelectionMatches, search, searchKeymap } from "@codemirror/search";
import { Compartment, EditorState, type Extension } from "@codemirror/state";
import {
  EditorView,
  crosshairCursor,
  drawSelection,
  dropCursor,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
  rectangularSelection,
} from "@codemirror/view";
import { tags } from "@lezer/highlight";
import { t, type MessageKey } from "../i18n";
import { detectIndent, type EditorLanguage } from "./editor-text";

/** The syntax mode, filled in after its parser loads. */
export const languageSlot = new Compartment();

/** Each mode is its own chunk, loaded the first time a file of that kind opens. */
const LOADERS: Record<EditorLanguage, () => Promise<Extension>> = {
  markdown: () => import("@codemirror/lang-markdown").then((mod) => mod.markdown()),
  json: () => import("@codemirror/lang-json").then((mod) => mod.json()),
  yaml: () => import("@codemirror/lang-yaml").then((mod) => mod.yaml()),
  toml: () => import("@codemirror/legacy-modes/mode/toml").then((mod) => StreamLanguage.define(mod.toml)),
  javascript: () => import("@codemirror/lang-javascript").then((mod) => mod.javascript()),
  jsx: () => import("@codemirror/lang-javascript").then((mod) => mod.javascript({ jsx: true })),
  typescript: () => import("@codemirror/lang-javascript").then((mod) => mod.javascript({ typescript: true })),
  tsx: () => import("@codemirror/lang-javascript").then((mod) => mod.javascript({ typescript: true, jsx: true })),
  rust: () => import("@codemirror/lang-rust").then((mod) => mod.rust()),
  python: () => import("@codemirror/lang-python").then((mod) => mod.python()),
  shell: () => import("@codemirror/legacy-modes/mode/shell").then((mod) => StreamLanguage.define(mod.shell)),
};

export const loadLanguage = (language: EditorLanguage): Promise<Extension> => LOADERS[language]();

/** CodeMirror's own words, in the app's language. */
const PHRASES: Record<string, MessageKey> = {
  Find: "editor.find",
  Replace: "editor.replace",
  next: "editor.next",
  previous: "editor.previous",
  all: "editor.all",
  "match case": "editor.matchCase",
  regexp: "editor.regexp",
  "by word": "editor.byWord",
  replace: "editor.replaceOne",
  "replace all": "editor.replaceAll",
  close: "editor.close",
  "Go to line": "editor.goToLine",
  go: "editor.go",
  "current match": "editor.currentMatch",
  "on line": "editor.onLine",
  "replaced match on line $": "editor.replacedOnLine",
  "replaced $ matches": "editor.replacedCount",
  "Folded lines": "editor.foldedLines",
  "Unfolded lines": "editor.unfoldedLines",
  to: "editor.to",
  "folded code": "editor.foldedCode",
  unfold: "editor.unfold",
  "Fold line": "editor.foldLine",
  "Unfold line": "editor.unfoldLine",
  "Control character": "editor.controlCharacter",
  "Selection deleted": "editor.selectionDeleted",
};

const phrases = () => Object.fromEntries(Object.entries(PHRASES).map(([phrase, key]) => [phrase, t(key)]));

/** Syntax colours from the design tokens, so every theme and mode follows on its own. */
const highlightStyle = HighlightStyle.define([
  { tag: tags.keyword, color: "var(--color-syntax-keyword)" },
  { tag: tags.comment, color: "var(--color-syntax-comment)", fontStyle: "italic" },
  { tag: [tags.string, tags.regexp, tags.character, tags.monospace], color: "var(--color-syntax-string)" },
  { tag: [tags.number, tags.bool, tags.null, tags.atom, tags.unit], color: "var(--color-syntax-number)" },
  { tag: [tags.typeName, tags.className, tags.namespace], color: "var(--color-syntax-type)" },
  { tag: tags.variableName, color: "var(--color-syntax-variable)" },
  { tag: [tags.function(tags.variableName), tags.function(tags.propertyName)], color: "var(--color-syntax-function)" },
  { tag: [tags.propertyName, tags.attributeName, tags.labelName], color: "var(--color-syntax-property)" },
  { tag: tags.operator, color: "var(--color-syntax-operator)" },
  { tag: tags.heading, fontWeight: "600", color: "var(--color-syntax-keyword)" },
  { tag: tags.strong, fontWeight: "600" },
  { tag: tags.emphasis, fontStyle: "italic" },
  { tag: tags.strikethrough, textDecoration: "line-through" },
  { tag: [tags.link, tags.url], color: "var(--color-syntax-operator)", textDecoration: "underline" },
  { tag: [tags.processingInstruction, tags.meta, tags.punctuation], color: "var(--color-syntax-punctuation)" },
  { tag: tags.invalid, color: "var(--color-syntax-invalid)", textDecoration: "underline wavy" },
]);

const theme = EditorView.theme({
  "&": {
    height: "100%",
    color: "var(--color-term-text)",
    backgroundColor: "var(--color-term-bg)",
    font: "var(--text-terminal)",
  },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { fontFamily: "var(--font-mono)", lineHeight: "1.5" },
  ".cm-content": { caretColor: "var(--color-text)", padding: "var(--space-2) 0" },
  ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--color-text)", borderLeftWidth: "2px" },
  "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection": {
    backgroundColor: "var(--color-press)",
  },
  ".cm-activeLine": { backgroundColor: "var(--color-panel)" },
  ".cm-gutters": {
    backgroundColor: "var(--color-term-bg)",
    color: "var(--color-text-3)",
    borderRight: "1px solid var(--color-line)",
  },
  ".cm-activeLineGutter": { backgroundColor: "var(--color-panel)", color: "var(--color-text-2)" },
  ".cm-foldPlaceholder": {
    backgroundColor: "var(--color-raised)",
    border: "1px solid var(--color-line-strong)",
    color: "var(--color-text-2)",
    borderRadius: "var(--radius-xs)",
  },
  "&.cm-focused .cm-matchingBracket": { backgroundColor: "var(--color-hover)", outline: "1px solid var(--color-line-strong)" },
  "&.cm-focused .cm-nonmatchingBracket": { outline: "1px solid var(--color-status-error)" },
  ".cm-searchMatch": { backgroundColor: "var(--color-hover)", outline: "1px solid var(--color-line-strong)" },
  ".cm-searchMatch.cm-searchMatch-selected": { outline: "2px solid var(--color-focus)" },
  ".cm-selectionMatch": { backgroundColor: "var(--color-hover)" },
  ".cm-panels": {
    backgroundColor: "var(--color-panel)",
    color: "var(--color-text)",
    font: "var(--text-label)",
  },
  ".cm-panels.cm-panels-top": { borderBottom: "1px solid var(--color-line)" },
  ".cm-panels.cm-panels-bottom": { borderTop: "1px solid var(--color-line)" },
  ".cm-panel.cm-search": { padding: "var(--space-2) var(--space-3)" },
  ".cm-panel.cm-search label": { font: "var(--text-caption)", color: "var(--color-text-2)" },
  ".cm-textfield": {
    backgroundColor: "var(--color-canvas)",
    color: "var(--color-text)",
    border: "1px solid var(--color-line-strong)",
    borderRadius: "var(--radius-xs)",
    font: "var(--text-meta)",
    padding: "2px var(--space-2)",
  },
  ".cm-button": {
    backgroundImage: "none",
    backgroundColor: "var(--color-raised)",
    color: "var(--color-text)",
    border: "1px solid var(--color-line)",
    borderRadius: "var(--radius-xs)",
    font: "var(--text-caption)",
  },
  ".cm-button:active": { backgroundColor: "var(--color-press)" },
  ".cm-textfield:focus-visible, .cm-button:focus-visible, .cm-panel input[type=checkbox]:focus-visible": {
    outline: "2px solid var(--color-focus)",
    outlineOffset: "2px",
  },
  ".cm-panel.cm-search [name=close]": { color: "var(--color-text-2)" },
  ".cm-tooltip": {
    backgroundColor: "var(--color-panel)",
    color: "var(--color-text)",
    border: "1px solid var(--color-line-strong)",
  },
});

export interface EditorOptions {
  /** Set for a file the tab cannot save: too large, or lines that end in different ways. */
  readOnly: boolean;
}

/**
 * A fresh editor state for a file's text. The text is kept exactly as given: nothing is trimmed,
 * reindented or normalized, and the language and indent settings only shape what is typed next.
 */
export function createEditorState(text: string, options: EditorOptions): EditorState {
  const indent = detectIndent(text);
  return EditorState.create({
    doc: text,
    extensions: [
      lineNumbers(),
      highlightActiveLineGutter(),
      highlightSpecialChars(),
      history(),
      foldGutter(),
      drawSelection(),
      dropCursor(),
      EditorState.allowMultipleSelections.of(true),
      indentOnInput(),
      syntaxHighlighting(highlightStyle),
      bracketMatching(),
      closeBrackets(),
      rectangularSelection(),
      crosshairCursor(),
      highlightActiveLine(),
      highlightSelectionMatches(),
      search({ top: true }),
      indentUnit.of(indent.unit),
      EditorState.tabSize.of(indent.tabSize),
      EditorState.readOnly.of(options.readOnly),
      EditorState.phrases.of(phrases()),
      keymap.of([
        ...closeBracketsKeymap,
        ...defaultKeymap,
        ...searchKeymap,
        ...historyKeymap,
        ...foldKeymap,
        indentWithTab,
      ]),
      languageSlot.of([]),
      theme,
    ],
  });
}
