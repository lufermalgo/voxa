import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Locale, translations } from "../i18n";

interface RecorderPillProps {
  status: string;
  label?: string;
  uiLocale: Locale;
}

export const RecorderPill = ({ status, label: customLabel, uiLocale }: RecorderPillProps) => {
  const [duration, setDuration] = useState(0);
  const isRecording = status === "recording";
  const isLoading = status === "loading" || status === "loading_whisper" || status === "loading_llama";
  const t = translations[uiLocale];

  useEffect(() => {
    let interval: number;
    if (isRecording) {
      interval = window.setInterval(() => {
        setDuration(d => d + 1);
      }, 1000);
    } else {
      setDuration(0);
    }
    return () => clearInterval(interval);
  }, [isRecording]);

  const formatTime = (seconds: number) => {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
  };

  const handleStop = () => {
    invoke("stop_recording");
  };

  const handleStart = () => {
    invoke("start_recording");
  };

  if (isLoading) {
    const defaultLabel = status === "loading_whisper" || status === "loading_llama" || status === "loading"
      ? t.processing
      : t.loading;
    const label = customLabel || defaultLabel;
    
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
    const label = t.processing;
    return (
      <div className="animate-in fade-in zoom-in-95 duration-500">
        <div className="bg-primary h-7 px-3 rounded-voxa flex items-center justify-center gap-2 shadow-2xl relative overflow-hidden">
          <div className="absolute inset-0 bg-white/10 animate-pulse" />
          <div className="w-3.5 h-3.5 border-2 border-white/30 border-t-white rounded-full animate-spin relative z-10" />
          <span className="text-[10px] font-bold text-white tracking-voxa-label uppercase font-manrope relative z-10 whitespace-nowrap">{label}</span>
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
          
          <button 
            onClick={() => invoke("stop_recording")}
            className="flex-shrink-0 flex items-center justify-center text-white/70 hover:text-white transition-colors cursor-pointer group"
          >
            <span className="material-symbols-outlined !text-[20px] group-hover:scale-110 transition-transform">close</span>
          </button>

          <div className="flex items-center gap-[2px] h-5">
            {[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8].map((delay, i) => (
              <div 
                key={i}
                className={`w-[2px] rounded-full animate-wave-recording ${
                  i < 3 || i > 14 ? 'bg-white/60' : 
                  i === 3 || i === 15 ? 'bg-white/80' : 'bg-white'
                }`}
                style={{ 
                  animationDelay: `${delay}s`,
                  height: i === 9 ? '100%' : i % 2 === 0 ? '50%' : '75%' 
                }} 
              />
            ))}
          </div>

          <button 
            onClick={handleStop}
            className="flex-shrink-0 flex items-center justify-center text-white/90 hover:text-white transition-colors cursor-pointer group"
          >
            <span className="material-symbols-outlined !text-[20px] material-symbols-fill group-hover:scale-110 transition-transform">stop</span>
          </button>

          <div className="absolute -top-6 left-1/2 -translate-x-1/2 opacity-0 group-hover:opacity-100 transition-opacity">
             <span className="text-[9px] font-mono text-white/60 tracking-wider font-bold">{formatTime(duration)}</span>
          </div>
        </div>
      </div>
    );
  }

  // IDLE STATE - Solid Violet Pill (6px height)
  return (
    <div className="animate-in fade-in zoom-in-95 duration-500">
      <div
        onClick={handleStart}
        className="bg-primary h-[6px] w-[40px] rounded-voxa shadow-lg hover:shadow-xl cursor-pointer transition-all hover:scale-110 hover:h-[8px]"
      />
    </div>
  );
};

