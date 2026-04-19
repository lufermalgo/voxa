import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Locale, translations } from "../i18n";
import { useAudioLevel } from "../hooks/useAudioLevel";
import { useRecordingDuration } from "../hooks/useRecordingDuration";
import { AppInfo } from "../hooks/useTranscription";

const PILL_WINDOW_HEIGHT_NORMAL  = 80;
const PILL_WINDOW_HEIGHT_WARNING = 220; // pill (28px) + gap (8px) + card (~90px) + padding

interface RecorderPillProps {
  status: string;
  label?: string;
  uiLocale: Locale;
  appInfo?: AppInfo | null;
}

export const RecorderPill = ({ status, label: customLabel, uiLocale, appInfo }: RecorderPillProps) => {
  const isIdle      = status === "idle";
  const isRecording = status === "recording";
  const isLoading   = status === "loading" || status === "loading_whisper" || status === "loading_llama";
  const isProcessing = status === "processing" || status === "refining";
  const isDone      = status === "done";
  const isActive    = !isIdle; // anything that isn't idle = pill is "inflated"

  const t = translations[uiLocale];

  const barHeights = useAudioLevel(isRecording);
  const { progress, isWarning, timeRemaining } = useRecordingDuration(isRecording);

  const prevIsWarningRef = useRef(false);

  // Expand the window upward when warning fires so the popup card is visible.
  useEffect(() => {
    if (isWarning === prevIsWarningRef.current) return;
    prevIsWarningRef.current = isWarning;

    const win = getCurrentWindow();
    win.setSize(new LogicalSize(300, isWarning ? PILL_WINDOW_HEIGHT_WARNING : PILL_WINDOW_HEIGHT_NORMAL));
  }, [isWarning]);

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

  const handleStop   = () => invoke("stop_and_transcribe");
  const handleCancel = () => invoke("cancel_recording");

  if (isLoading) {
    const label = customLabel || t.processing;
    return (
      <div className="animate-in fade-in zoom-in-95 duration-500">
        <div className="bg-[#0A0A0A]/80 backdrop-blur-[40px] h-12 px-4 rounded-[24px] flex items-center gap-2 shadow-[0_20px_50px_rgba(0,0,0,0.5)] border border-white/10 relative overflow-hidden">
          <div className="w-3.5 h-3.5 border-2 border-white/30 border-t-white rounded-full animate-spin" />
          <span className="text-[10px] font-bold text-white tracking-[0.2em] uppercase font-manrope whitespace-nowrap">{label}</span>
        </div>
      </div>
    );
  }

  if (status === "processing" || status === "refining") {
    return (
      <div className="animate-in fade-in zoom-in-95 duration-500">
        <div className="bg-[#0A0A0A]/80 backdrop-blur-[40px] h-12 px-4 rounded-[24px] flex items-center justify-center gap-2 shadow-[0_20px_50px_rgba(0,0,0,0.5)] border border-white/10 relative overflow-hidden">
          <div className="absolute inset-0 bg-white/10 animate-pulse" />
          <div className="w-3.5 h-3.5 border-2 border-white/30 border-t-white rounded-full animate-spin relative z-10" />
          <span className="text-[10px] font-bold text-white tracking-[0.2em] uppercase font-manrope relative z-10 whitespace-nowrap">{t.processing}</span>
        </div>
      </div>
    );
  }

  if (status === "done") {
    return (
      <div className="animate-in fade-in zoom-in-95 duration-500">
        <div className="bg-[#0A0A0A]/80 backdrop-blur-[40px] h-12 px-4 rounded-[24px] flex items-center gap-2 shadow-[0_20px_50px_rgba(0,0,0,0.5)] border border-white/10 relative overflow-hidden">
          <span className="material-symbols-outlined text-primary !text-[18px] animate-in zoom-in duration-300">check_circle</span>
          <span className="text-[10px] font-bold text-white tracking-[0.2em] uppercase font-manrope">{t.sent}</span>
        </div>
      </div>
    );
  }

  return (
    <div className={`relative flex items-center justify-center ${isIdle ? "pointer-events-none" : ""}`}>

        {/* ── Warning popup card (above pill) — fades in at 80% ── */}
        {isWarning && (
          <div className="animate-in fade-in slide-in-from-bottom-2 duration-300 w-[268px] bg-[#1c1c1e] border border-white/10 rounded-2xl px-4 py-3 shadow-2xl">
            <div className="flex items-start gap-3">
              <span className="material-symbols-outlined text-amber-400 !text-[20px] mt-0.5 flex-shrink-0">warning</span>
              <div className="flex flex-col gap-1">
                <p className="text-[11px] font-bold text-white font-manrope leading-tight">
                  {t.recording_limit_popup_title}
                </p>
                <p className="text-[10px] text-white/60 font-manrope leading-snug">
                  {t.recording_limit_popup_desc.replace('{s}', String(timeRemaining))}
                </p>
              </div>
            </div>
          </div>
        )}

        {/* ── Pill (bottom, anchored to Dock) ── */}
        <div
          className={`h-7 px-3 rounded-voxa flex items-center gap-2 shadow-2xl relative overflow-hidden justify-center min-w-[100px] transition-colors duration-700 ${
            isWarning ? 'bg-amber-600' : 'bg-primary'
          }`}
        >
          <div className="absolute inset-0 bg-white/5" />

            {/* Amber top accent bar */}
            <div className="h-[3px] w-full bg-gradient-to-r from-amber-600 via-amber-400 to-amber-600" />

            <div className="px-5 py-4 flex flex-col gap-3">

              {/* Header row */}
              <div className="flex items-center gap-3">
                {/* Icon container */}
                <div className="w-9 h-9 rounded-xl bg-amber-500/15 border border-amber-500/20 flex items-center justify-center flex-shrink-0">
                  <span className="material-symbols-outlined text-amber-400 !text-[20px] material-symbols-fill">timer</span>
                </div>
              )}
            </div>
          )}

          {/* Stop */}
          <button
            onClick={handleStop}
            className="flex-shrink-0 flex items-center justify-center text-white/90 hover:text-white transition-colors cursor-pointer group z-10"
          >
            <span className="material-symbols-outlined !text-[20px] material-symbols-fill group-hover:scale-110 transition-transform">stop</span>
          </button>

          {/* Duration progress bar — visible from the start, turns amber at 80% */}
          <div
            className={`absolute bottom-0 left-0 ${isWarning ? 'h-[5px] bg-amber-400' : 'h-[3px] bg-white/50'}`}
            style={{ width: `${progress * 100}%`, transition: 'width 200ms linear, height 700ms ease, background-color 700ms ease' }}
          />
        </div>

      </div>
    </div>
  );
};
