import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

interface RecorderPillProps {
  status: string;
}

export const RecorderPill = ({ status }: RecorderPillProps) => {
  const [duration, setDuration] = useState(0);
  const isRecording = status === "recording";
  const isLoading = status === "loading";

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
    return (
      <div className="animate-in fade-in zoom-in-95 duration-500">
        <div className="obsidian-glass h-8 px-4 rounded-full flex items-center gap-2 shadow-2xl border border-white/10 relative overflow-hidden ring-1 ring-black/50">
          <div className="w-3 h-3 border-2 border-primary/30 border-t-primary rounded-full animate-spin" />
          <span className="text-[9px] font-bold text-primary tracking-[0.15em] uppercase">Loading...</span>
        </div>
      </div>
    );
  }

  if (isRecording) {
    return (
      <div className="animate-in fade-in zoom-in-95 duration-500">
        <div className="obsidian-glass h-10 px-4 rounded-full flex items-center gap-3 shadow-2xl border border-white/10 relative overflow-hidden ring-1 ring-black/50 justify-center">
          <div className="absolute inset-0 bg-gradient-to-r from-primary/5 via-transparent to-primary/5" />
          
          <button 
            onClick={() => invoke("stop_recording")}
            className="flex-shrink-0 flex items-center justify-center text-on-surface-variant hover:text-on-surface transition-colors cursor-pointer group"
          >
            <span className="material-symbols-outlined !text-[20px] group-hover:scale-110 transition-transform">close</span>
          </button>

          <div className="flex items-center gap-[2px] h-5 waveform-aura">
            {[0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8].map((delay, i) => (
              <div 
                key={i}
                className={`w-[2px] rounded-full animate-wave-recording ${
                  i < 3 || i > 14 ? 'bg-tertiary-fixed-dim/40' : 
                  i === 3 || i === 15 ? 'bg-tertiary-fixed-dim/60' : 'bg-primary-fixed'
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
            className="flex-shrink-0 flex items-center justify-center text-primary-container hover:text-primary transition-colors cursor-pointer group"
          >
            <span className="material-symbols-outlined !text-[20px] material-symbols-fill group-hover:scale-110 transition-transform">stop</span>
          </button>

          <div className="absolute -top-6 left-1/2 -translate-x-1/2 opacity-0 group-hover:opacity-100 transition-opacity">
             <span className="text-[9px] font-mono text-primary/60 tracking-wider font-bold">{formatTime(duration)}</span>
          </div>
        </div>
      </div>
    );
  }

  // IDLE STATE - Exact from mock
  return (
    <div className="animate-in fade-in zoom-in-95 duration-500">
      <div 
        onClick={handleStart}
        className="obsidian-glass h-[6px] w-[40px] rounded-full flex items-center justify-center shadow-2xl border border-white/5 relative cursor-pointer overflow-hidden transition-transform"
      >
        <div className="absolute inset-0 bg-gradient-to-r from-primary/10 via-transparent to-primary/10 rounded-full" />
        
        <div className="flex items-center gap-[1.5px] h-full waveform-aura">
          {[0.2, 0.4, 0.6, 0.8, 1.0].map((delay, i) => (
            <div 
              key={i}
              className={`w-[1.5px] h-[1px] rounded-full animate-wave ${
                i === 0 || i === 4 ? 'bg-primary-fixed/50' : 'bg-primary-fixed'
              }`}
              style={{ animationDelay: `${delay}s` }} 
            />
          ))}
        </div>
      </div>
    </div>
  );
};
