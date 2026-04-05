import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

export const TrayMenu = () => {
  const [selectedProfile, setSelectedProfile] = useState("Technical Transcription");
  const [language, setLanguage] = useState("EN");

  const closeMenu = () => getCurrentWindow().hide();
  const openSettings = (tab?: string) => {
    invoke("show_settings", { tab });
    closeMenu();
  };

  const handleQuit = () => invoke("exit_app");

  return (
    <div className="w-full h-full bg-[#131314]/80 mac-blur rounded-[24px] shadow-2xl border border-white/10 flex flex-col overflow-hidden ring-1 ring-black/50 text-on-surface select-none font-body">
      {/* Header Section */}
      <div className="px-5 py-4 flex items-center justify-between">
        <div className="flex items-center space-x-3">
          <div className="relative flex items-center justify-center">
            <span className="material-symbols-outlined text-primary material-symbols-fill">graphic_eq</span>
            <div className="absolute -top-0.5 -right-0.5 w-2.5 h-2.5 bg-white rounded-full shadow-[0_0_8px_rgba(255,255,255,0.8)] border border-[#131314]" />
          </div>
          <span className="font-headline font-bold text-[15px] tracking-tight text-white">Voxa is Ready</span>
        </div>
      </div>

      <div className="h-[1px] bg-white/5 mx-3" />

      <div className="flex-1 overflow-y-auto custom-scrollbar">
        {/* Section 1: Profiles */}
        <div className="px-3 py-3">
          <div className="px-3 py-1.5 mb-1">
            <span className="font-label text-[11px] uppercase tracking-[0.15rem] text-on-surface-variant font-bold">Profiles</span>
          </div>
          <div className="space-y-1">
            <ProfileItem 
              label="Technical Transcription" 
              icon="person" 
              active={selectedProfile === "Technical Transcription"} 
              onClick={() => setSelectedProfile("Technical Transcription")} 
            />
            <ProfileItem 
              label="Creative Writing" 
              icon="edit_note" 
              active={selectedProfile === "Creative Writing"} 
              onClick={() => setSelectedProfile("Creative Writing")} 
            />
            <ProfileItem 
              label="Board Meeting" 
              icon="meeting_room" 
              active={selectedProfile === "Board Meeting"} 
              onClick={() => setSelectedProfile("Board Meeting")} 
            />
          </div>
        </div>

        <div className="h-[1px] bg-white/5 mx-3 my-1" />

        {/* Section 2: Language */}
        <div className="px-3 py-3">
          <div className="px-3 py-1.5 mb-1">
            <span className="font-label text-[11px] uppercase tracking-[0.15rem] text-on-surface-variant font-bold">Language</span>
          </div>
          <div className="px-3 flex items-center">
            <div className="flex w-full bg-[#1c1b1c] rounded-full p-1 border border-white/5">
              <button 
                onClick={() => setLanguage("EN")}
                className={`flex-1 py-1.5 rounded-full text-[12px] transition-all ${language === 'EN' ? 'bg-white/10 shadow-sm text-white font-bold tracking-wider' : 'text-on-surface-variant hover:text-on-surface font-medium'}`}
              >
                EN
              </button>
              <button 
                onClick={() => setLanguage("ES")}
                className={`flex-1 py-1.5 rounded-full text-[12px] transition-all ${language === 'ES' ? 'bg-white/10 shadow-sm text-white font-bold tracking-wider' : 'text-on-surface-variant hover:text-on-surface font-medium'}`}
              >
                ES
              </button>
            </div>
          </div>
        </div>

        <div className="h-[1px] bg-white/5 mx-3 my-1" />

        {/* Footer Actions */}
        <div className="px-3 pb-3 pt-1">
          <div className="space-y-1">
            <ActionItem icon="settings" label="Settings..." shortcut="⌘," onClick={() => openSettings('general')} />
            <ActionItem icon="menu_book" label="Dictionary..." shortcut="⌘D" onClick={() => openSettings('dictionary')} />
            <div className="h-[1px] bg-white/5 mx-2 my-2" />
            <ActionItem icon="power_settings_new" label="Quit" shortcut="⌘Q" onClick={handleQuit} isDestructive />
          </div>
        </div>
      </div>

      {/* Bottom Texture Tip */}
      <div className="bg-[#cbbeff]/5 px-4 py-3 flex items-center justify-center border-t border-white/[0.02]">
        <span className="text-[10px] text-[#cbbeff]/60 font-bold tracking-[0.2em] uppercase">Voxa Version 2.4.0</span>
      </div>
    </div>
  );
};

const ProfileItem = ({ label, icon, active, onClick }: { label: string, icon: string, active: boolean, onClick: () => void }) => (
  <div 
    onClick={onClick}
    className={`flex items-center justify-between px-3 py-2.5 rounded-xl cursor-pointer transition-all duration-200 group ${active ? 'bg-[#cbbeff]/10' : 'hover:bg-white/5'}`}
  >
    <div className="flex items-center space-x-4">
      <span className={`material-symbols-outlined ${active ? 'text-primary material-symbols-fill' : 'text-on-surface-variant group-hover:text-on-surface'}`}>{icon}</span>
      <span className={`text-[13px] font-semibold ${active ? 'text-primary' : 'text-on-surface-variant group-hover:text-on-surface'}`}>{label}</span>
    </div>
    {active && <span className="material-symbols-outlined text-primary text-[18px] material-symbols-fill">check_circle</span>}
  </div>
);

const ActionItem = ({ icon, label, shortcut, onClick, isDestructive }: { icon: string, label: string, shortcut: string, onClick: () => void, isDestructive?: boolean }) => (
  <button 
    onClick={onClick}
    className={`w-full flex items-center justify-between px-3 py-2.5 rounded-xl transition-all group ${isDestructive ? 'hover:bg-red-400/10' : 'hover:bg-white/5'}`}
  >
    <div className="flex items-center space-x-4">
      <span className={`material-symbols-outlined text-on-surface-variant transition-colors ${isDestructive ? 'group-hover:text-red-400' : 'group-hover:text-on-surface'}`}>{icon}</span>
      <span className={`text-[13px] font-semibold transition-colors ${isDestructive ? 'text-red-400 group-hover:text-red-400' : 'text-on-surface group-hover:text-on-surface'}`}>{label}</span>
    </div>
    <span className={`text-[11px] font-mono transition-colors text-white/20 ${isDestructive ? 'text-red-400 group-hover:text-red-400' : 'group-hover:text-on-surface'}`}>{shortcut}</span>
  </button>
);
