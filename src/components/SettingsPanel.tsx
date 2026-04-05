import { useState, useEffect, useRef } from "react";
import { useSettings } from "../hooks/useSettings";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface SettingsPanelProps {
  onClose: () => void;
  initialTab?: string;
}

export function SettingsPanel({ onClose, initialTab = "general" }: SettingsPanelProps) {
  const { settings, profiles, dictionary, updateSetting, addWord, removeWord, loading } = useSettings();
  const [micDevices, setMicDevices] = useState<string[]>([]);
  const [isCapturingShortcut, setIsCapturingShortcut] = useState(false);
  const [newWord, setNewWord] = useState("");
  const [activeTab, setActiveTab] = useState(initialTab);
  const scrollContainerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setActiveTab(initialTab);
  }, [initialTab]);

  useEffect(() => {
    invoke<string[]>("get_audio_devices")
      .then(setMicDevices)
      .catch(console.error);
  }, []);

  const handleShortcutKeyDown = (e: React.KeyboardEvent) => {
    if (!isCapturingShortcut) return;
    e.preventDefault();
    e.stopPropagation();

    const modifiers = [];
    if (e.altKey) modifiers.push("Alt");
    if (e.ctrlKey) modifiers.push("Control");
    if (e.metaKey) modifiers.push("Command");
    if (e.shiftKey) modifiers.push("Shift");

    const key = e.key === " " ? "Space" : e.key.charAt(0).toUpperCase() + e.key.slice(1);
    if (["Alt", "Control", "Command", "Shift", "Meta"].includes(key)) return;

    const shortcut = [...modifiers, key].join("+");
    updateSetting("global_shortcut", shortcut);
    setIsCapturingShortcut(false);
  };

  if (loading || !settings) return (
    <div className="h-full w-full flex items-center justify-center bg-[#0A0A0A]">
      <div className="text-white/40 font-bold tracking-widest text-[10px] animate-pulse">VOXA CORE INITIALIZING...</div>
    </div>
  );

  const tabs = [
    { id: "general", label: "General", icon: "⚙️" },
    { id: "engine", label: "The Engine", icon: "🧠" },
    { id: "dictionary", label: "Dictionary", icon: "📚" },
    { id: "shortcut", label: "Shortcut", icon: "⌨️" },
  ];

  return (
    <div className="h-screen w-screen bg-[#0A0A0A] flex flex-col overflow-hidden select-none" onKeyDown={handleShortcutKeyDown} tabIndex={0}>
      <header className="flex items-center justify-between p-6 border-b border-white/5 bg-white/[0.01]">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 rounded-xl bg-white flex items-center justify-center font-black text-black">V</div>
          <div>
            <h1 className="text-sm font-bold text-white tracking-tight">Voxa Control Center</h1>
            <p className="text-[10px] text-white/30 font-bold uppercase tracking-widest">System Voice Intelligence</p>
          </div>
        </div>
        <button onClick={onClose} className="p-2 hover:bg-white/5 rounded-full transition-colors text-white/20 hover:text-white">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
        </button>
      </header>

      <div className="flex-1 flex overflow-hidden">
        {/* Sidebar */}
        <aside className="w-64 border-r border-white/5 p-4 space-y-1 bg-white/[0.005]">
          {tabs.map(tab => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`w-full flex items-center gap-3 px-4 py-3 rounded-xl transition-all duration-300 text-sm font-medium
                ${activeTab === tab.id 
                  ? 'bg-white/5 text-white shadow-xl shadow-black/50' 
                  : 'text-white/30 hover:bg-white/[0.02] hover:text-white/60'}`}
            >
              <span className="opacity-60">{tab.icon}</span>
              {tab.label}
            </button>
          ))}
        </aside>

        {/* Content */}
        <main ref={scrollContainerRef} className="flex-1 overflow-y-auto p-12 scroll-smooth bg-gradient-to-br from-transparent to-white/[0.01]">
          <div className="max-w-2xl mx-auto space-y-16 animate-in fade-in slide-in-from-right-4 duration-500">
            
            {/* SECTION: GENERAL */}
            {(activeTab === "general" || activeTab === "all") && (
              <section className="space-y-8">
                <div className="space-y-1">
                  <h3 className="text-lg font-bold text-white">General Settings</h3>
                  <p className="text-xs text-white/40">Basic configuration for audio and language.</p>
                </div>

                <div className="grid gap-6">
                  <div className="space-y-3">
                    <label className="text-[10px] font-black text-white/20 uppercase tracking-[0.2em] ml-1">Input Source</label>
                    <div className="relative group">
                      <select 
                        value={settings.mic_id}
                        onChange={(e) => updateSetting("mic_id", e.target.value)}
                        className="w-full bg-white/[0.03] border border-white/5 p-4 rounded-2xl text-white/80 text-sm appearance-none focus:outline-none focus:border-white/20 transition-all cursor-pointer group-hover:bg-white/[0.05]"
                      >
                        <option value="none">System Default Microphone</option>
                        {micDevices.map(id => <option key={id} value={id}>{id}</option>)}
                      </select>
                      <div className="absolute right-4 top-1/2 -translate-y-1/2 pointer-events-none opacity-40">
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3"><polyline points="6 9 12 15 18 9"></polyline></svg>
                      </div>
                    </div>
                  </div>

                  <div className="space-y-3">
                    <label className="text-[10px] font-black text-white/20 uppercase tracking-[0.2em] ml-1">Transcription Language</label>
                    <div className="grid grid-cols-2 gap-3">
                      {['es', 'en'].map(lang => (
                        <button
                          key={lang}
                          onClick={() => updateSetting("language", lang)}
                          className={`py-4 rounded-2xl border transition-all text-xs font-bold uppercase tracking-[0.2em]
                            ${settings.language === lang 
                              ? 'bg-white text-black border-transparent shadow-2xl shadow-white/10' 
                              : 'bg-white/[0.02] border-white/5 text-white/30 hover:bg-white/5'}`}
                        >
                          {lang === 'es' ? 'Spanish' : 'English'}
                        </button>
                      ))}
                    </div>
                  </div>
                </div>
              </section>
            )}

            {/* SECTION: ENGINE */}
            {(activeTab === "engine" || activeTab === "all") && (
              <section className="space-y-8">
                <div className="space-y-1">
                  <h3 className="text-lg font-bold text-white">Transformation Engine</h3>
                  <p className="text-xs text-white/40">Choose how the AI refines your spoken words.</p>
                </div>

                <div className="grid gap-3">
                  {profiles.map(profile => (
                    <button
                      key={profile.id}
                      onClick={() => updateSetting("active_profile_id", profile.id.toString())}
                      className={`p-5 rounded-3xl border transition-all text-left flex justify-between items-center group
                        ${settings.active_profile_id === profile.id.toString() 
                          ? 'bg-white/5 border-white/20 scale-[1.02] shadow-2xl shadow-black' 
                          : 'bg-white/[0.01] border-white/5 hover:border-white/10 hover:bg-white/[0.02]'}`}
                    >
                      <div className="flex-1">
                        <div className="flex items-center gap-2">
                          <span className={`text-xs font-black uppercase tracking-widest ${settings.active_profile_id === profile.id.toString() ? 'text-white' : 'text-white/40'}`}>
                            {profile.name}
                          </span>
                          {settings.active_profile_id === profile.id.toString() && (
                             <span className="px-2 py-0.5 rounded-full bg-white/10 text-[8px] font-black text-white/60">ACTIVE</span>
                          )}
                        </div>
                        <div className="text-[10px] text-white/20 mt-1 line-clamp-2 italic leading-relaxed">
                          "{profile.system_prompt || "Transcribes exactly what you say without any stylistic changes."}"
                        </div>
                      </div>
                      {settings.active_profile_id === profile.id.toString() && (
                        <div className="w-1.5 h-1.5 rounded-full bg-white animate-pulse ml-4 shadow-[0_0_10px_white]" />
                      )}
                    </button>
                  ))}
                </div>
              </section>
            )}

            {/* SECTION: DICTIONARY */}
            {(activeTab === "dictionary" || activeTab === "all") && (
              <section className="space-y-8">
                <div className="space-y-1">
                  <h3 className="text-lg font-bold text-white">Custom Dictionary</h3>
                  <p className="text-xs text-white/40">Teach Voxa technical terms and proper names.</p>
                </div>

                <div className="p-8 rounded-[2.5rem] bg-white/[0.02] border border-white/5 space-y-8">
                  <div className="flex flex-wrap gap-2.5">
                    {dictionary.map(word => (
                      <span 
                        key={word} 
                        className="bg-white/5 border border-white/10 px-4 py-2 rounded-2xl text-[10px] font-bold text-white/60 flex items-center gap-3 group hover:border-white/20 hover:text-white transition-all"
                      >
                        {word}
                        <button onClick={() => removeWord(word)} className="text-white/10 hover:text-red-400 transition-colors">
                          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
                        </button>
                      </span>
                    ))}
                    {dictionary.length === 0 && <p className="text-xs text-white/10 italic">Your internal vocabulary is currently empty.</p>}
                  </div>
                  
                  <div className="flex gap-3 pt-4 border-t border-white/5">
                    <input 
                      type="text"
                      placeholder="Add technical term or name..."
                      value={newWord}
                      onChange={(e) => setNewWord(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter' && newWord.trim()) {
                          addWord(newWord.trim());
                          setNewWord("");
                        }
                      }}
                      className="flex-1 bg-white/[0.03] border border-white/5 p-4 rounded-2xl text-white/80 text-sm focus:outline-none focus:border-white/20 transition-all placeholder:text-white/10"
                    />
                    <button 
                      onClick={() => { if (newWord.trim()) { addWord(newWord.trim()); setNewWord(""); } }}
                      className="bg-white text-black px-8 rounded-2xl text-xs font-black uppercase tracking-widest hover:bg-white/90 transition-all active:scale-95"
                    >
                      Add
                    </button>
                  </div>
                </div>
              </section>
            )}

            {/* SECTION: SHORTCUT */}
            {(activeTab === "shortcut" || activeTab === "all") && (
              <section className="space-y-8 pb-12">
                <div className="space-y-1">
                  <h3 className="text-lg font-bold text-white">System Activation</h3>
                  <p className="text-xs text-white/40">Global shortcut to toggle dictation.</p>
                </div>

                <div className="flex flex-col gap-6">
                  <button
                    onClick={() => setIsCapturingShortcut(true)}
                    className={`group w-full p-10 rounded-[2.5rem] border-2 border-dashed transition-all flex flex-col items-center gap-4
                      ${isCapturingShortcut 
                         ? 'bg-white/10 border-white/40 text-white animate-pulse' 
                         : 'bg-white/[0.01] border-white/5 text-white/60 hover:bg-white/[0.02] hover:border-white/10'}`}
                  >
                    <div className="text-[10px] font-black uppercase tracking-[0.4em] opacity-40">Current Hotkey</div>
                    <div className="text-4xl font-black tracking-tighter text-white font-mono">
                      {isCapturingShortcut ? "Listening..." : settings.global_shortcut.replace("Command", "⌘").replace("Alt", "⌥").replace("Shift", "⇧").replace("Control", "⌃")}
                    </div>
                    {!isCapturingShortcut && (
                      <div className="mt-4 px-6 py-2 rounded-full border border-white/5 text-[9px] font-black uppercase tracking-widest group-hover:bg-white group-hover:text-black transition-all">
                        Click to rebind
                      </div>
                    )}
                  </button>

                  <div className="p-6 rounded-3xl bg-amber-500/5 border border-amber-500/10 flex gap-4">
                    <span className="text-xl">⚠️</span>
                    <div className="space-y-1">
                      <p className="text-xs font-bold text-amber-200/80 uppercase tracking-widest">Shortcut Hint</p>
                      <p className="text-[10px] text-amber-200/40 leading-relaxed uppercase font-bold tracking-wider">
                        Use <span className="text-amber-200/60">Alt+Space</span> or <span className="text-amber-200/60">Cmd+L</span> for the fast Wispr-like experience.
                      </p>
                    </div>
                  </div>
                </div>
              </section>
            )}

          </div>
        </main>
      </div>

      <footer className="p-6 border-t border-white/5 bg-white/[0.01] flex justify-between items-center">
        <div className="text-[9px] text-white/10 font-bold uppercase tracking-[0.4em]">Engine v0.1.0 • macOS Native</div>
        <div className="flex gap-4">
           {/* Add more footer actions if needed */}
        </div>
      </footer>
    </div>
  );
}

export default SettingsPanel;
