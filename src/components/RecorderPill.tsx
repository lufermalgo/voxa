import { invoke } from "@tauri-apps/api/core";
import { Locale, translations } from "../i18n";
import { useAudioLevel } from "../hooks/useAudioLevel";

interface RecorderPillProps {
  status: string;
  label?: string;
  uiLocale: Locale;
}

export const RecorderPill = ({ status, label: customLabel, uiLocale }: RecorderPillProps) => {
  const isRecording = status === "recording";
  const isIdle = status === "idle" || status === "";
  const isLoading = status === "loading" || status === "loading_whisper" || status === "loading_llama";
  const t = translations[uiLocale];


  const barHeights = useAudioLevel(isRecording);

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
      <div className="animate-in fade-in zoom-in-95 duration-500">
        <div className="bg-primary h-7 px-3 rounded-voxa flex items-center gap-2 shadow-2xl relative overflow-hidden justify-center min-w-[100px]">
          <div className="absolute inset-0 bg-white/5" />

          {/* X — Cancel: discard recording, back to idle */}
          <button
            onClick={handleCancel}
            className="flex-shrink-0 flex items-center justify-center text-white/70 hover:text-white transition-colors cursor-pointer group z-10"
          >
            <span className="material-symbols-outlined !text-[20px] group-hover:scale-110 transition-transform">close</span>
          </button>

          {/* Waveform — VAD-reactive */}
          <div className="flex items-center gap-[2px] h-5 z-10">
            {barHeights.map((height, i) => (
              <div
                key={i}
                className={`w-[2px] rounded-full ${
                  i < 3 || i > 14 ? 'bg-white/60' :
                  i === 3 || i === 15 ? 'bg-white/80' : 'bg-white'
                }`}
                style={{
                  height: `${height}px`,
                  transition: 'height 40ms ease-out',
                }}
              />
            ))}
          </div>

          {/* Stop — process and transcribe */}
          <button
            onClick={handleStop}
            className="flex-shrink-0 flex items-center justify-center text-white/90 hover:text-white transition-colors cursor-pointer group z-10"
          >
            <span className="material-symbols-outlined !text-[20px] material-symbols-fill group-hover:scale-110 transition-transform">stop</span>
          </button>
        </div>
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
