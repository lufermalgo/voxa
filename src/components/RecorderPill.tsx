import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Locale, translations } from "../i18n";
import { useAudioLevel } from "../hooks/useAudioLevel";
import { useRecordingDuration } from "../hooks/useRecordingDuration";
import { AppInfo } from "../hooks/useTranscription";
import type { Profile } from "../hooks/useProfiles";
import { ProfilePicker } from "./ProfilePicker";

interface RecorderPillProps {
  status: string;
  label?: string;
  uiLocale: Locale;
  appInfo?: AppInfo | null;
  profiles?: Profile[];
  refinedText?: string;
  rawText?: string;
}

export const RecorderPill = ({ status, label: customLabel, uiLocale, appInfo, profiles = [], refinedText }: RecorderPillProps) => {
  const isIdle       = status === "idle";
  const isRecording  = status === "recording";
  const isLoading    = status === "loading" || status === "loading_whisper" || status === "loading_llama";
  const isProcessing = status === "processing" || status === "refining";
  const isDone       = status === "done";
  const isActive     = !isIdle;

  const t = translations[uiLocale];

  const barHeights = useAudioLevel(isRecording);
  const { progress, isWarning, timeRemaining } = useRecordingDuration(isRecording);

  const prevIsWarningRef = useRef(false);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [showCorrectionCard, setShowCorrectionCard] = useState(false);
  const [correctedText, setCorrectedText] = useState("");
  const [correctionSubmitting, setCorrectionSubmitting] = useState(false);
  const correctionTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (!isRecording) {
      if (prevIsWarningRef.current) {
        prevIsWarningRef.current = false;
        invoke("set_pill_warning_mode", { expand: false }).catch(() => {});
      }
      return;
    }
    if (isWarning === prevIsWarningRef.current) return;
    prevIsWarningRef.current = isWarning;
    invoke("set_pill_warning_mode", { expand: isWarning }).catch(() => {});
  }, [isWarning, isRecording]);

  useEffect(() => {
    if (isDone && refinedText) {
      setShowCorrectionCard(false);
      setCorrectedText("");
      correctionTimerRef.current = setTimeout(() => setShowCorrectionCard(false), 30000);
    } else if (!isDone) {
      setShowCorrectionCard(false);
      if (correctionTimerRef.current) clearTimeout(correctionTimerRef.current);
    }
    return () => { if (correctionTimerRef.current) clearTimeout(correctionTimerRef.current); };
  }, [isDone, refinedText]);

  const handleSubmitCorrection = async () => {
    if (!correctedText.trim() || !refinedText) return;
    setCorrectionSubmitting(true);
    try {
      const settings = await invoke<Record<string, string>>("get_settings");
      const profileId = parseInt(settings.active_profile_id ?? "1", 10);
      await invoke("submit_correction", { profileId, originalText: refinedText, correctedText: correctedText.trim() });
    } catch (e) {
      console.error("Correction submit failed:", e);
    } finally {
      setCorrectionSubmitting(false);
      setShowCorrectionCard(false);
    }
  };

  const handleStop   = () => invoke("stop_and_transcribe");
  const handleCancel = () => invoke("cancel_recording");

  // Background color per state
  const bgColor = isWarning
    ? "bg-amber-600/80"
    : isIdle
      ? "bg-primary/60"
      : "bg-[#0A0A0A]/80";

  // Size: idle = tiny bar, active = full pill
  const pillSize = isIdle
    ? "h-[6px] w-[40px]"
    : "h-12 w-[200px] px-2";

  const pillStyle = {
    transition: "all 500ms cubic-bezier(0.34, 1.56, 0.64, 1)",
    transformOrigin: "center",
  };

  return (
    <div className={`relative flex items-center justify-center ${isIdle ? "pointer-events-none" : ""}`}>

      {/* Warning card — floats above pill via absolute positioning */}
      {isWarning && isRecording && (
        <div className="absolute bottom-[calc(100%+12px)] left-1/2 -translate-x-1/2 w-[300px] z-50 animate-in fade-in slide-in-from-bottom-2 duration-400">
          <div className="relative rounded-[20px] overflow-hidden bg-[#0A0A0A]/90 backdrop-blur-[40px] border border-amber-500/30 shadow-[0_20px_60px_rgba(0,0,0,0.6)]">
            <div className="h-[3px] w-full bg-gradient-to-r from-amber-600 via-amber-400 to-amber-600" />
            <div className="px-5 py-4 flex flex-col gap-3">
              <div className="flex items-center gap-3">
                <div className="w-9 h-9 rounded-xl bg-amber-500/15 border border-amber-500/20 flex items-center justify-center flex-shrink-0">
                  <span className="material-symbols-outlined text-amber-400 !text-[20px] material-symbols-fill">timer</span>
                </div>
                <p className="text-[13px] font-black text-white font-manrope leading-tight tracking-tight">
                  {t.recording_limit_popup_title}
                </p>
              </div>
              <div className="flex items-center gap-3">
                <div className="w-9 h-9 rounded-xl bg-amber-500/10 border border-amber-500/15 flex items-center justify-center flex-shrink-0">
                  <span className="text-[16px] font-black text-amber-400 font-manrope tabular-nums leading-none">
                    {timeRemaining}
                  </span>
                </div>
                <p className="text-[11px] text-white/60 font-manrope leading-snug">
                  {t.recording_limit_popup_desc.replace('{s}', String(timeRemaining))}
                </p>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Profile picker — floats above pill */}
      {pickerOpen && (
        <div className="absolute bottom-[calc(100%+8px)] left-1/2 -translate-x-1/2 z-50">
          <ProfilePicker
            profiles={profiles}
            currentProfileName={""}
            onSelect={async (name) => {
              try {
                await invoke("set_manual_profile_override", { profileName: name });
              } catch (e) {
                console.error("Failed to set profile override:", e);
              }
              setPickerOpen(false);
            }}
            onClose={() => setPickerOpen(false)}
          />
        </div>
      )}

      {/* THE PILL — single element, always present, morphs between states */}
      <div
        className={`
          rounded-[24px] flex items-center justify-center gap-1 relative overflow-hidden
          ${pillSize}
          ${bgColor}
          ${isActive ? "shadow-[0_20px_50px_rgba(0,0,0,0.5)] border border-white/10 backdrop-blur-[40px]" : "shadow-lg"}
        `}
        style={pillStyle}
      >
        {/* Subtle inner glow overlay when active */}
        {isActive && (
          <div className="absolute inset-0 bg-white/5 pointer-events-none" />
        )}

        {/* LOADING */}
        {isLoading && (
          <>
            <div className="w-5 h-5 border-2 border-white/30 border-t-white rounded-full animate-spin z-10 flex-shrink-0" />
            <span className="text-[12px] font-bold text-white tracking-[0.15em] uppercase font-manrope whitespace-nowrap z-10">
              {customLabel || t.processing}
            </span>
          </>
        )}

        {/* RECORDING */}
        {isRecording && (
          <>
            <button
              onClick={handleCancel}
              className="flex-shrink-0 flex items-center justify-center text-white/70 hover:text-white transition-colors cursor-pointer group z-10"
            >
              <span className="material-symbols-outlined !text-[28px] group-hover:scale-110 transition-transform">close</span>
            </button>

            <div className="flex items-center gap-[2px] h-8 z-10 flex-shrink-0">
              {barHeights.map((height, i) => (
                <div
                  key={i}
                  className={`w-[2.5px] rounded-full ${
                    i < 2 || i > 15 ? "bg-white/40" :
                    i < 4 || i > 13 ? "bg-white/60" :
                    i < 6 || i > 11 ? "bg-white/80" :
                    "bg-white"
                  }`}
                  style={{ height: `${height}px`, transition: "height 40ms ease-out" }}
                />
              ))}
            </div>

            {appInfo && (
              <div title={appInfo.name} className="flex-shrink-0 z-10">
                {appInfo.icon ? (
                  <img src={`data:image/png;base64,${appInfo.icon}`} alt={appInfo.name} className="w-8 h-8 rounded-[6px] opacity-90" />
                ) : (
                  <div className="w-8 h-8 rounded-[6px] bg-white/20 flex items-center justify-center opacity-90">
                    <span className="text-[12px] font-bold text-white">{appInfo.name.charAt(0).toUpperCase()}</span>
                  </div>
                )}
              </div>
            )}

            <button
              onClick={handleStop}
              className="flex-shrink-0 flex items-center justify-center text-white/90 hover:text-white transition-colors cursor-pointer group z-10"
            >
              <span className="material-symbols-outlined !text-[28px] material-symbols-fill group-hover:scale-110 transition-transform">stop</span>
            </button>

            <div
              className={`absolute bottom-0 left-0 ${isWarning ? "h-[5px] bg-amber-400" : "h-[3px] bg-white/50"}`}
              style={{ width: `${progress * 100}%`, transition: "width 200ms linear, height 700ms ease, background-color 700ms ease" }}
            />
          </>
        )}

        {/* PROCESSING / REFINING */}
        {isProcessing && (
          <>
            <div className="w-5 h-5 border-2 border-white/30 border-t-white/80 rounded-full animate-spin z-10 flex-shrink-0" />
            <span className="ml-2 text-[12px] font-bold text-white tracking-[0.15em] uppercase font-manrope z-10 whitespace-nowrap">
              {t.processing}
            </span>
          </>
        )}

        {/* DONE */}
        {isDone && (
          <>
            <span className="material-symbols-outlined text-primary !text-[24px] z-10 animate-in zoom-in duration-300">check_circle</span>
            <span className="text-[12px] font-bold text-white tracking-[0.15em] uppercase font-manrope z-10">
              {t.sent}
            </span>
            {refinedText && (
              <button
                onClick={() => setShowCorrectionCard(v => !v)}
                className="flex-shrink-0 flex items-center justify-center text-white/40 hover:text-white/80 transition-colors cursor-pointer z-10 ml-1"
                title="Corrección"
              >
                <span className="material-symbols-outlined !text-[18px]">edit</span>
              </button>
            )}
          </>
        )}
      </div>

      {/* CORRECTION CARD — floats above pill */}
      {showCorrectionCard && isDone && (
        <div className="absolute bottom-[calc(100%+12px)] left-1/2 -translate-x-1/2 w-[320px] z-50 animate-in fade-in slide-in-from-bottom-2 duration-300">
          <div className="relative rounded-[20px] overflow-hidden bg-[#0A0A0A]/90 backdrop-blur-[40px] border border-white/10 shadow-[0_20px_60px_rgba(0,0,0,0.6)]">
            <div className="h-[2px] w-full bg-gradient-to-r from-primary/60 via-primary to-primary/60" />
            <div className="px-5 py-4 space-y-3">
              <p className="text-[11px] font-black text-white/60 uppercase tracking-widest">Corrección</p>
              <p className="text-[11px] text-white/40 leading-snug">Pega el texto corregido para que Voxa aprenda tu preferencia de formato.</p>
              <textarea
                className="w-full bg-white/5 border border-white/10 rounded-xl p-3 text-[12px] text-white/80 resize-none focus:outline-none focus:border-primary/40 leading-relaxed"
                rows={4}
                placeholder="Texto corregido..."
                value={correctedText}
                onChange={e => setCorrectedText(e.target.value)}
                autoFocus
              />
              <div className="flex gap-2">
                <button
                  onClick={handleSubmitCorrection}
                  disabled={correctionSubmitting || !correctedText.trim()}
                  className="flex-1 bg-primary text-white py-2.5 rounded-xl text-[11px] font-black uppercase tracking-widest hover:bg-primary/90 active:scale-95 transition-all disabled:opacity-40 disabled:cursor-not-allowed"
                >
                  {correctionSubmitting ? "..." : "Enviar"}
                </button>
                <button
                  onClick={() => setShowCorrectionCard(false)}
                  className="px-4 bg-white/5 text-white/40 py-2.5 rounded-xl text-[11px] font-black uppercase tracking-widest hover:bg-white/10 transition-all"
                >
                  Cancelar
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
