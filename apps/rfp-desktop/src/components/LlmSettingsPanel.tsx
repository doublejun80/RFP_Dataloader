import { BrainCircuit, KeyRound, Play, Save } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import type {
  DocumentSummary,
  LlmProvider,
  LlmSettings,
  SaveLlmSettingsRequest,
} from "../lib/types";

interface LlmSettingsPanelProps {
  document: DocumentSummary | null;
  error: string | null;
  loading: boolean;
  onRun: () => void;
  onSave: (request: SaveLlmSettingsRequest) => void;
  settings: LlmSettings | null;
}

const DEFAULT_MODELS: Record<LlmProvider, string> = {
  openai: "gpt-5.5",
  gemini: "gemini-2.5-pro",
};

const MODEL_OPTIONS: Record<LlmProvider, Array<{ label: string; value: string }>> = {
  openai: [
    { label: "GPT-5.5 (권장, 최신)", value: "gpt-5.5" },
    { label: "GPT-5.5 Pro (고정밀)", value: "gpt-5.5-pro" },
    { label: "GPT-5.4 Mini (비용/속도)", value: "gpt-5.4-mini" },
    { label: "GPT-5.4 Nano (최저비용)", value: "gpt-5.4-nano" },
  ],
  gemini: [
    { label: "Gemini 2.5 Pro (권장)", value: "gemini-2.5-pro" },
    { label: "Gemini 2.5 Flash (비용/속도)", value: "gemini-2.5-flash" },
    { label: "Gemini Flash Latest (최신 alias)", value: "gemini-flash-latest" },
  ],
};

function modelForProvider(provider: LlmProvider, candidate: string): string {
  return MODEL_OPTIONS[provider].some((option) => option.value === candidate)
    ? candidate
    : DEFAULT_MODELS[provider];
}

export function LlmSettingsPanel({
  document,
  error,
  loading,
  onRun,
  onSave,
  settings,
}: LlmSettingsPanelProps) {
  const [enabled, setEnabled] = useState(false);
  const [offlineMode, setOfflineMode] = useState(true);
  const [provider, setProvider] = useState<LlmProvider>("openai");
  const [model, setModel] = useState(DEFAULT_MODELS.openai);
  const [apiKey, setApiKey] = useState("");

  useEffect(() => {
    if (!settings) {
      return;
    }

    setEnabled(settings.enabled);
    setOfflineMode(settings.offlineMode);
    setProvider(settings.provider);
    setModel(modelForProvider(settings.provider, settings.model));
    setApiKey("");
  }, [settings]);

  const statusText = useMemo(() => {
    if (!settings) {
      return "LLM 설정 확인 중";
    }

    if (!settings.enabled) {
      return "LLM 구조화 꺼짐";
    }

    if (settings.offlineMode) {
      return "LLM 오프라인 모드";
    }

    if (!settings.apiKeyConfigured) {
      return "LLM API 키 없음";
    }

    return "LLM 구조화 준비됨";
  }, [settings]);

  const canRun =
    Boolean(document) &&
    Boolean(settings?.enabled) &&
    !settings?.offlineMode &&
    Boolean(settings?.apiKeyConfigured) &&
    !loading;

  function handleProviderChange(nextProvider: LlmProvider) {
    setProvider(nextProvider);
    setModel(DEFAULT_MODELS[nextProvider]);
  }

  return (
    <section className="llm-panel" aria-label="LLM 구조화 설정">
      <div className="llm-panel-header">
        <div>
          <h3>
            <BrainCircuit aria-hidden="true" size={18} />
            LLM 구조화
          </h3>
          <p>{statusText}</p>
        </div>
        <button disabled={!canRun} onClick={onRun} type="button">
          <Play aria-hidden="true" size={16} />
          LLM 구조화 실행
        </button>
      </div>

      <div className="llm-controls">
        <label className="llm-toggle">
          <input
            aria-label="LLM 사용"
            checked={enabled}
            onChange={(event) => setEnabled(event.target.checked)}
            type="checkbox"
          />
          <span>LLM 사용</span>
        </label>
        <label className="llm-toggle">
          <input
            aria-label="오프라인 모드"
            checked={offlineMode}
            onChange={(event) => setOfflineMode(event.target.checked)}
            type="checkbox"
          />
          <span>오프라인 모드</span>
        </label>
        <label className="llm-field">
          <span>제공자</span>
          <select
            aria-label="LLM 제공자"
            onChange={(event) => handleProviderChange(event.target.value as LlmProvider)}
            value={provider}
          >
            <option value="openai">OpenAI</option>
            <option value="gemini">Gemini</option>
          </select>
        </label>
        <label className="llm-field">
          <span>모델</span>
          <select
            aria-label="LLM 모델"
            onChange={(event) => setModel(event.target.value)}
            value={model}
          >
            {MODEL_OPTIONS[provider].map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <label className="llm-field llm-field--key">
          <span>API 키</span>
          <input
            aria-label="API 키"
            onChange={(event) => setApiKey(event.target.value)}
            placeholder={settings?.apiKeyConfigured ? "저장된 키 유지" : "키 입력"}
            type="password"
            value={apiKey}
          />
        </label>
        <button
          disabled={loading}
          onClick={() =>
            onSave({
              enabled,
              offlineMode,
              provider,
              model,
              apiKey: apiKey.trim() || null,
            })
          }
          type="button"
        >
          <Save aria-hidden="true" size={16} />
          LLM 설정 저장
        </button>
      </div>

      {settings?.apiKeyConfigured ? (
        <p className="llm-key-status">
          <KeyRound aria-hidden="true" size={14} />
          API 키는 OS 키체인 참조로 저장됨
        </p>
      ) : null}

      {error ? (
        <p className="llm-error" role="alert">
          {error}
        </p>
      ) : null}
    </section>
  );
}
