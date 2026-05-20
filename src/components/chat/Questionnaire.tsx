import { useEffect, useMemo, useState } from "react";
import { Icon } from "@iconify/react";
import type { QuestionAnswer } from "../../types";

type QuestionMode = "single_choice" | "multiple_choice";

type QuestionOption = {
  id: string;
  label: string;
  description?: string;
};

type QuestionPayload = {
  question: string;
  mode: QuestionMode;
  options: QuestionOption[];
};

type QuestionStep = {
  id: string;
  item: QuestionItem;
  payload: QuestionPayload;
  answer?: QuestionAnswer;
};

export type QuestionItem = {
  id: string;
  argsPretty?: string;
  status: "running" | "done" | "error";
  isError?: boolean;
  answered?: boolean;
  answer?: string;
  summary?: string;
};

type Props = {
  questions: QuestionItem[];
  onAnswerQuestion: (
    toolCallId: string,
    answers: QuestionAnswer[],
    options?: { stopQuestions?: boolean },
  ) => void | Promise<void>;
  disabled: boolean;
  allowStopQuestions?: boolean;
};

function stringFromUnknown(value: unknown): string | null {
  if (typeof value === "string") {
    const trimmed = value.trim();
    return trimmed.length > 0 ? trimmed : null;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return null;
}

function normalizeQuestionMode(
  value: unknown,
  record: Record<string, unknown>,
): QuestionMode {
  const raw =
    typeof value === "string"
      ? value.toLowerCase().replace(/[\s-]+/g, "_")
      : "";
  if (
    raw === "multiple" ||
    raw === "multiple_choice" ||
    raw === "multi_select" ||
    record.multiple === true ||
    record.allowMultiple === true
  ) {
    return "multiple_choice";
  }
  return "single_choice";
}

function questionOptionFromValue(
  value: unknown,
  index: number,
): QuestionOption | null {
  if (typeof value === "string" || typeof value === "number") {
    const label = String(value).trim();
    return label ? { id: String(index), label } : null;
  }
  if (!value || typeof value !== "object") return null;
  const record = value as Record<string, unknown>;
  const label =
    stringFromUnknown(record.label) ??
    stringFromUnknown(record.text) ??
    stringFromUnknown(record.value) ??
    stringFromUnknown(record.id);
  if (!label) return null;
  return {
    id:
      stringFromUnknown(record.id) ??
      stringFromUnknown(record.value) ??
      String(index),
    label,
    description: stringFromUnknown(record.description) ?? undefined,
  };
}

function questionPayloadFromRecord(
  record: Record<string, unknown>,
): QuestionPayload | null {
  const rawOptions = Array.isArray(record.options)
    ? record.options
    : Array.isArray(record.choices)
      ? record.choices
      : [];
  const options = rawOptions
    .map(questionOptionFromValue)
    .filter((item): item is QuestionOption => item !== null);
  const question =
    stringFromUnknown(record.question) ??
    stringFromUnknown(record.prompt) ??
    stringFromUnknown(record.title);
  if (!question) return null;
  return {
    question,
    mode: normalizeQuestionMode(
      record.type ?? record.mode ?? record.kind,
      record,
    ),
    options,
  };
}

function parseQuestionArgs(argsPretty?: string): QuestionPayload[] {
  if (!argsPretty) return [];
  try {
    const parsed = JSON.parse(argsPretty) as unknown;
    if (!parsed || typeof parsed !== "object") return [];
    const record = parsed as Record<string, unknown>;
    if (Array.isArray(record.questions)) {
      return record.questions
        .map((item) =>
          item && typeof item === "object"
            ? questionPayloadFromRecord(item as Record<string, unknown>)
            : null,
        )
        .filter((item): item is QuestionPayload => item !== null);
    }
    const single = questionPayloadFromRecord(record);
    return single ? [single] : [];
  } catch {
    return [];
  }
}

function answerForStep(
  raw: string | undefined,
  payload: QuestionPayload,
  index: number,
): QuestionAnswer | undefined {
  const trimmed = raw?.trim();
  if (!trimmed) return undefined;
  const lines = trimmed.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
  const line = lines.length > 1 ? lines[index] : trimmed;
  if (!line) return undefined;
  const parts = payload.mode === "multiple_choice"
    ? line.split(",").map((part) => part.trim()).filter(Boolean)
    : [line];
  return parts.length > 0 ? parts : undefined;
}

function questionStepsFromItems(questions: QuestionItem[]): QuestionStep[] {
  return questions.flatMap((item) =>
    parseQuestionArgs(item.argsPretty).map((payload, index) => ({
      id: `${item.id}:${index}`,
      item,
      payload,
      answer: answerForStep(item.answer, payload, index),
    })),
  );
}

function cleanPrompt(raw: string): string {
  return raw
    .replace(
      /\s*[(\[\-—–,:]+\s*(plusieurs\s+choix\s+possibles?|choix\s+multiples?|multiple\s+choice|select\s+all\s+that\s+apply)[)\]]*\s*$/i,
      "",
    )
    .replace(
      /\s*(plusieurs\s+choix\s+possibles?|choix\s+multiples?)\s*$/i,
      "",
    )
    .trim();
}

type AnswerState = {
  selected: Record<string, Set<string>>;
  custom: Record<string, string>;
};

function emptyAnswerState(): AnswerState {
  return { selected: {}, custom: {} };
}

function restoreAnswerState(
  steps: QuestionStep[],
): AnswerState {
  const state = emptyAnswerState();
  for (const step of steps) {
    const answer = step.answer;
    if (!answer || answer.length === 0) continue;
    const restored = restoreAnswer(answer, step.payload);
    if (restored.selected.size > 0) {
      state.selected[step.id] = restored.selected;
    }
    if (restored.custom) {
      state.custom[step.id] = restored.custom;
    }
  }
  return state;
}

function restoreAnswer(
  answer: QuestionAnswer,
  payload: QuestionPayload,
): { selected: Set<string>; custom: string } {
  if (payload.mode === "single_choice") {
    const value = answer[0] ?? "";
    const match = payload.options.find((option) => option.label === value);
    return {
      selected: new Set(match ? [match.id] : []),
      custom: match ? "" : value,
    };
  }

  const selected = new Set<string>();
  const custom: string[] = [];
  for (const value of answer) {
    const match = payload.options.find((option) => option.label === value);
    if (!match) {
      custom.push(value);
      continue;
    }
    selected.add(match.id);
  }

  return {
    selected,
    custom: custom.join(", "),
  };
}

export function Questionnaire({
  questions,
  onAnswerQuestion,
  disabled,
  allowStopQuestions = false,
}: Props) {
  const steps = useMemo(() => questionStepsFromItems(questions), [questions]);
  const persistedAnswered =
    questions.length > 0 && questions.every((q) => q.answered === true);
  const questionStateKey = useMemo(
    () =>
      steps
        .map((step) =>
          [
            step.id,
            step.payload.question,
            step.payload.mode,
            step.payload.options.map((option) => option.label).join("\u001d"),
            step.item.answered === true ? "1" : "0",
            step.answer?.join("\u001c") ?? "",
          ].join("\u001e"),
        )
        .join("\u001f"),
    [steps],
  );
  const restoredAnswerState = useMemo(
    () =>
      persistedAnswered ? restoreAnswerState(steps) : emptyAnswerState(),
    [persistedAnswered, steps],
  );
  const [index, setIndex] = useState(0);
  const [selected, setSelected] = useState<Record<string, Set<string>>>(
    () => restoredAnswerState.selected,
  );
  const [custom, setCustom] = useState<Record<string, string>>(
    () => restoredAnswerState.custom,
  );
  const [sent, setSent] = useState(persistedAnswered);
  const [open, setOpen] = useState(!persistedAnswered);

  useEffect(() => {
    setIndex(0);
    setSelected(restoredAnswerState.selected);
    setCustom(restoredAnswerState.custom);
    setSent(persistedAnswered);
    setOpen(!persistedAnswered);
  }, [questionStateKey, persistedAnswered]);

  const anyRunning = questions.some((q) => q.status === "running");
  const anyError = questions.some((q) => q.isError);
  const noParsedPayload = steps.length === 0;
  if (noParsedPayload && anyRunning) {
    return (
      <div className="tool-card tool-card--questionnaire-loading">
        <div className="tool-card__head">
          <span className="tool-card__spinner" />
          <span className="tool-card__title">
            {questions.length > 1 ? "Preparing questions" : "Preparing question"}
          </span>
        </div>
      </div>
    );
  }
  if (steps.length === 0) {
    return (
      <div className="tool-card tool-card--questionnaire-loading">
        <div className="tool-card__head">
          <span className={anyError ? "tool-card__err-dot" : "tool-card__glyph"} />
          <span className="tool-card__title">
            {questions[0]?.summary ?? "Question unavailable"}
          </span>
        </div>
      </div>
    );
  }

  const clampedIndex = Math.max(0, Math.min(index, steps.length - 1));
  const current = steps[clampedIndex];
  const payload = current.payload;

  const multiple = payload?.mode === "multiple_choice";
  const stepDisabled = disabled || sent || anyError || !payload;

  const currentSelected = selected[current.id] ?? new Set<string>();
  const currentCustom = custom[current.id] ?? "";

  const goTo = (next: number) => {
    if (next < 0 || next >= steps.length) return;
    setIndex(next);
  };

  const onOptionClick = (option: QuestionOption) => {
    if (stepDisabled || !payload) return;
    if (multiple) {
      setSelected((prev) => {
        const cur = new Set(prev[current.id] ?? []);
        if (cur.has(option.id)) cur.delete(option.id);
        else cur.add(option.id);
        return { ...prev, [current.id]: cur };
      });
    } else {
      setSelected((prev) => ({
        ...prev,
        [current.id]: new Set([option.id]),
      }));
      setCustom((prev) => ({ ...prev, [current.id]: "" }));
    }
  };

  const answerFor = (qIndex: number): QuestionAnswer | null => {
    const step = steps[qIndex];
    if (!step) return null;
    const p = step.payload;
    const customText = (custom[step.id] ?? "").trim();
    const picks = selected[step.id] ?? new Set<string>();
    const opts = p.options.filter((o) => picks.has(o.id));
    const isMulti = p.mode === "multiple_choice";
    if (isMulti) {
      const parts = opts.map((o) => o.label);
      if (customText) parts.push(customText);
      if (parts.length === 0) return null;
      return parts;
    }
    if (customText) return [customText];
    if (opts.length === 0) return null;
    return [opts[0].label];
  };

  const allAnswered = steps.every((_, i) => answerFor(i) !== null);
  const isLastStep = clampedIndex === steps.length - 1;
  const canSend = !disabled && !sent && !anyError && allAnswered;

  const send = (stopQuestions = false) => {
    if (!canSend) return;
    const answers: QuestionAnswer[] = [];
    for (let i = 0; i < steps.length; i++) {
      const answer = answerFor(i);
      if (answer === null) return;
      answers.push(answer);
    }
    if (!onAnswerQuestion) return;
    void onAnswerQuestion(current.item.id, answers, { stopQuestions });
    setSent(true);
    setOpen(false);
  };

  const rawPrompt = payload?.question ?? current.item.summary ?? "Question";
  const prompt = cleanPrompt(rawPrompt);

  return (
    <div
      className="tool-card tool-card--questionnaire"
      data-error={current.item.isError ? "true" : "false"}
      data-sent={sent ? "true" : "false"}
    >
      {sent && (
        <div
          className="tool-card__head"
          onClick={() => setOpen((v) => !v)}
        >
          <span className="tool-card__glyph">
            <Icon
              icon="solar:question-circle-linear"
              width={13}
              height={13}
            />
          </span>
          <span className="tool-card__title">
            {steps.length > 1 ? "Questions Answered" : "Question Answered"}
          </span>
          <span className="tool-card__caret" data-open={open ? "true" : "false"}>
            <Icon
              icon={
                open
                  ? "solar:alt-arrow-down-linear"
                  : "solar:alt-arrow-right-linear"
              }
              width={12}
              height={12}
            />
          </span>
        </div>
      )}
      {(!sent || open) && (
        <div className="tool-card__body questionnaire__body">
      <div className="questionnaire__prompt-row">
        <div className="question-tool__prompt">{prompt}</div>
        {steps.length > 1 && (
          <span className="questionnaire__count">
            {clampedIndex + 1} / {steps.length}
          </span>
        )}
      </div>
      {payload &&
        (payload.options.length ? (
          <div
            className="question-tool__options"
            role={multiple ? "group" : "radiogroup"}
          >
            {payload.options.map((option) => {
              const isSelected = currentSelected.has(option.id);
              return (
                <button
                  key={option.id}
                  type="button"
                  className="question-tool__option"
                  data-selected={isSelected ? "true" : "false"}
                  onClick={() => onOptionClick(option)}
                  role={multiple ? "checkbox" : "radio"}
                  aria-checked={isSelected}
                  disabled={stepDisabled}
                >
                  <span className="question-tool__option-copy">
                    <span className="question-tool__option-label">
                      {option.label}
                    </span>
                    {option.description && (
                      <span className="question-tool__option-desc">
                        {option.description}
                      </span>
                    )}
                  </span>
                  {multiple ? (
                    <span
                      className="question-tool__mark"
                      data-selected={isSelected ? "true" : "false"}
                      aria-hidden
                    >
                      {isSelected && (
                        <Icon
                          icon="solar:check-read-linear"
                          width={11}
                          height={11}
                        />
                      )}
                    </span>
                  ) : (
                    isSelected && (
                      <Icon
                        icon="solar:check-read-linear"
                        width={13}
                        height={13}
                        className="question-tool__check"
                      />
                    )
                  )}
                </button>
              );
            })}
          </div>
        ) : null)}
      <div className="questionnaire__custom">
        <input
          type="text"
          className="questionnaire__custom-input"
          placeholder="Or type your own answer…"
          value={currentCustom}
          onChange={(event) => {
            const value = event.target.value;
            setCustom((prev) => ({ ...prev, [current.id]: value }));
            if (!multiple && value.length > 0) {
              setSelected((prev) => {
                if (!prev[current.id] || prev[current.id].size === 0) return prev;
                return { ...prev, [current.id]: new Set() };
              });
            }
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter" && isLastStep && canSend) {
              event.preventDefault();
              send();
            }
          }}
          disabled={stepDisabled}
        />
      </div>
      {(!sent || steps.length > 1) && (
        <div className="questionnaire__footer">
          <div className="questionnaire__actions">
            {steps.length > 1 && (
              <button
                type="button"
                className="questionnaire__nav"
                onClick={() => goTo(clampedIndex - 1)}
                disabled={clampedIndex === 0}
              >
                Back
              </button>
            )}
            {!sent &&
              (isLastStep ? (
                <>
                  <button
                    type="button"
                    className="questionnaire__send"
                    onClick={() => send(false)}
                    disabled={!canSend}
                  >
                    <span className="questionnaire__send-label">Send</span>
                  </button>
                  {allowStopQuestions && (
                    <button
                      type="button"
                      className="questionnaire__send questionnaire__send--stop"
                      onClick={() => send(true)}
                      disabled={!canSend}
                    >
                      <span className="questionnaire__send-label">
                        Send and stop questions
                      </span>
                    </button>
                  )}
                </>
              ) : (
                <button
                  type="button"
                  className="questionnaire__nav"
                  data-primary="true"
                  onClick={() => goTo(clampedIndex + 1)}
                >
                  Next
                </button>
              ))}
            {sent && steps.length > 1 && (
              <button
                type="button"
                className="questionnaire__nav"
                data-primary="true"
                onClick={() => goTo(clampedIndex + 1)}
                disabled={clampedIndex === steps.length - 1}
              >
                Next
              </button>
            )}
          </div>
        </div>
      )}
        </div>
      )}
    </div>
  );
}
