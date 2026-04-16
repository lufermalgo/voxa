import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogicalSize, PhysicalPosition } from "@tauri-apps/api/dpi";
import { Locale, translations } from "../i18n";
import { useAudioLevel } from "../hooks/useAudioLevel";
import { useRecordingDuration } from "../hooks/useRecordingDuration";
import { AppInfo } from "../hooks/useTranscription";

const PILL_WINDOW_HEIGHT_NORMAL  = 100;
const PILL_WINDOW_HEIGHT_WARNING = 220; // card (~90px) + pill (28px) + gap + padding

interface RecorderPillProps {
  status: string;
  label?: string;
  uiLocale: Locale;
  appInfo?: AppInfo | null;
}

export const RecorderPill = ({ status, label: customLabel, uiLocale, appInfo }: RecorderPillProps) => {
  const isRecording = status === "recording";
  const isLoading = status === "loading" || status === "loading_whisper" || status === "loading_llama";
  const t = translations[uiLocale];


  const barHeights = useAudioLevel(isRecording);
  const { progress, isWarning, timeRemaining } = useRecordingDuration(isRecording);
  const originalPosRef = useRef<{ x: number; y: number } | null>(null);

  // Expand the window UPWARD so the popup card appears ABOVE the pill.
  // Store original physical position, move window up by the extra height
  // (scaled to physical pixels via scaleFactor), restore on dismiss.
  useEffect(() => {
    const win = getCurrentWindow();
    const extraHeight = PILL_WINDOW_HEIGHT_WARNING - PILL_WINDOW_HEIGHT_NORMAL;

    if (isWarning) {
      Promise.all([win.outerPosition(), win.scaleFactor()]).then(([pos, scale]) => {
        originalPosRef.current = { x: pos.x, y: pos.y };
        win.setPosition(new PhysicalPosition(pos.x, pos.y - Math.round(extraHeight * scale)));
        win.setSize(new LogicalSize(300, PILL_WINDOW_HEIGHT_WARNING));
      });
    } else if (originalPosRef.current) {
      win.setPosition(new PhysicalPosition(originalPosRef.current.x, originalPosRef.current.y));
      win.setSize(new LogicalSize(300, PILL_WINDOW_HEIGHT_NORMAL));
      originalPosRef.current = null;
    }
  }, [isWarning]);

  const handleStop = () => invoke("stop_and_transcribe");
  const handleCancel = () => invoke("cancel_recording");

  if (isLoading) {
    const label = customLabel || t.processing;
    return (
      <div className="animate-in fade-in zoom-in-95 duration-500">
        <div className="bg-primary h-7 px-3 rounded-voxa flex items-center gap-2 shadow-2xl relative overflow-hidden">
          <div className="w-3.5 h-3.5 border-2 border-white/30 border-t-white rounded-full animate-spin" />
          <span className="text-[10px] font-bold text-white tracking-voxa-label uppercase font-manrope whitespace-nowrap">{label}</span>
        </div>
      </div>
    );
  }

  if (status === "processing" || status === "refining") {
    return (
      <div className="animate-in fade-in zoom-in-95 duration-500">
        <div className="bg-primary h-7 px-3 rounded-voxa flex items-center justify-center gap-2 shadow-2xl relative overflow-hidden">
          <div className="absolute inset-0 bg-white/10 animate-pulse" />
          <div className="w-3.5 h-3.5 border-2 border-white/30 border-t-white rounded-full animate-spin relative z-10" />
          <span className="text-[10px] font-bold text-white tracking-voxa-label uppercase font-manrope relative z-10 whitespace-nowrap">{t.processing}</span>
        </div>
      </div>
    );
  }

  if (status === "done") {
    return (
      <div className="animate-in fade-in zoom-in-95 duration-500">
        <div className="bg-primary h-7 px-3 rounded-voxa flex items-center gap-2 shadow-2xl relative overflow-hidden">
          <span className="material-symbols-outlined text-white !text-[18px] animate-in zoom-in duration-300">check_circle</span>
          <span className="text-[10px] font-bold text-white tracking-voxa-label uppercase font-manrope">{t.sent}</span>
        </div>
      </div>
    );
  }

  if (isRecording) {
    return (
      // flex-col-reverse: card renders first in DOM = appears at TOP visually,
      // pill second = appears at BOTTOM. Combined with the window moving upward,
      // the card floats above the pill's original screen position.
      <div className="animate-in fade-in zoom-in-95 duration-500 flex flex-col-reverse items-center gap-2">

        {/* ── Pill (bottom) ── */}
        <div
          className={`h-7 px-3 rounded-voxa flex items-center gap-2 shadow-2xl relative overflow-hidden justify-center min-w-[100px] transition-colors duration-700 ${
            isWarning ? 'bg-amber-600' : 'bg-primary'
          }`}
        >
          <div className="absolute inset-0 bg-white/5" />

          {/* X — Cancel */}
          <button
            onClick={handleCancel}
            className="flex-shrink-0 flex items-center justify-center text-white/70 hover:text-white transition-colors cursor-pointer group z-10"
          >
            <span className="material-symbols-outlined !text-[20px] group-hover:scale-110 transition-transform">close</span>
          </button>

          {/* Waveform — always visible */}
          <div className="flex items-center gap-[2px] h-5 z-10">
            {barHeights.map((height, i) => (
              <div
                key={i}
                className={`w-[2px] rounded-full ${
                  i < 3 || i > 14 ? 'bg-white/60' :
                  i === 3 || i === 15 ? 'bg-white/80' : 'bg-white'
                }`}
                style={{ height: `${height}px`, transition: 'height 40ms ease-out' }}
              />
            ))}
          </div>

          {/* App icon */}
          {appInfo && (
            <div title={appInfo.name} className="flex-shrink-0 z-10">
              {appInfo.icon ? (
                <img src={`data:image/png;base64,${appInfo.icon}`} alt={appInfo.name} className="w-5 h-5 rounded-[4px] opacity-80" />
              ) : (
                <div className="w-5 h-5 rounded-[4px] bg-white/20 flex items-center justify-center opacity-80">
                  <span className="text-[9px] font-bold text-white">{appInfo.name.charAt(0).toUpperCase()}</span>
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
            className={`absolute bottom-0 left-0 rounded-b-voxa transition-all duration-700 ${
              isWarning ? 'h-[5px] bg-amber-400' : 'h-[3px] bg-white/55'
            }`}
            style={{ width: `${progress * 100}%`, transitionProperty: 'width, height, background-color' }}
          />
        </div>

        {/* ── Warning popup card (top) — fades in above the pill at 80% ── */}
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
      </div>
    );
  }

  // IDLE — non-interactive thin bar, clicks pass through to system
  return (
    <div className="animate-in fade-in zoom-in-95 duration-500 pointer-events-none">
      <div className="bg-primary h-[6px] w-[40px] rounded-voxa shadow-lg" />
    </div>
  );
};
