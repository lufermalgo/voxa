import { useRef, useEffect } from "react";
import type { Profile } from "../hooks/useProfiles";

interface ProfilePickerProps {
  profiles: Profile[];
  currentProfileName: string;
  onSelect: (name: string | null) => void;
  onClose: () => void;
}

export function ProfilePicker({ profiles, currentProfileName, onSelect, onClose }: ProfilePickerProps) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        onClose();
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [onClose]);

  return (
    <div
      ref={ref}
      className="absolute bottom-[calc(100%+8px)] left-1/2 -translate-x-1/2 z-50
                 bg-[#0A0A0A]/90 backdrop-blur-[40px] border border-white/10
                 rounded-[16px] shadow-[0_20px_60px_rgba(0,0,0,0.6)]
                 p-2 min-w-[160px] animate-in fade-in slide-in-from-bottom-2 duration-200"
    >
      {/* Auto option — clears manual override */}
      <button
        onClick={() => { onSelect(null); onClose(); }}
        className="w-full flex items-center gap-2 px-3 py-2 rounded-xl
                   text-[11px] font-bold text-white/60 hover:text-white
                   hover:bg-white/10 transition-colors"
      >
        <span className="material-symbols-outlined !text-[14px]">auto_awesome</span>
        Auto
      </button>

      <div className="h-px bg-white/10 my-1" />

      {profiles.map(profile => (
        <button
          key={profile.id}
          onClick={() => { onSelect(profile.name); onClose(); }}
          className={`w-full flex items-center gap-2 px-3 py-2 rounded-xl
                      text-[11px] font-bold transition-colors
                      ${profile.name === currentProfileName
                        ? "text-white bg-white/10"
                        : "text-white/60 hover:text-white hover:bg-white/10"}`}
        >
          <span className="material-symbols-outlined !text-[14px] material-symbols-fill">
            {profile.icon || "psychology"}
          </span>
          {profile.name}
        </button>
      ))}
    </div>
  );
}
