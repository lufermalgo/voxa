import { useState, useEffect, useRef } from "react";
import { useSettings } from "../hooks/useSettings";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Locale, translations } from "../i18n";

interface SettingsPanelProps {
  onClose: () => void;
  initialTab?: string;
  uiLocale: Locale;
}

interface AudioDevice {
  id: string;
  name: string;
}

export function SettingsPanel({ onClose, initialTab = "general", uiLocale }: SettingsPanelProps) {
  const t = translations[uiLocale];
  const { settings, profiles, dictionary, updateSetting, addWord, removeWord, updateProfile, createProfile, deleteProfile, loading } = useSettings();
  const [micDevices, setMicDevices] = useState<AudioDevice[]>([]);
  const [isCapturingShortcut, setIsCapturingShortcut] = useState(false);
  const [newWord, setNewWord] = useState("");
  const [activeTab, setActiveTab] = useState(initialTab === 'general' ? 'history' : initialTab);
  const [transcripts, setTranscripts] = useState<any[]>([]);
  
  // State for editing profiles
  const [editingProfileId, setEditingProfileId] = useState<number | null>(null);
  const [editName, setEditName] = useState("");
  const [editPrompt, setEditPrompt] = useState("");
  const [editIcon, setEditIcon] = useState("");

  // State for new profile
  const [isCreatingProfile, setIsCreatingProfile] = useState(false);
  const [newName, setNewName] = useState("");
  const [newPrompt, setNewPrompt] = useState("");
  const [newIcon, setNewIcon] = useState("psychology");

  const scrollContainerRef = useRef<HTMLDivElement>(null);

  const AVAILABLE_ICONS = [
    'star', 'forum', 'code', 'tune', 'description', 'psychology', 
    'edit_note', 'chat', 'terminal', 'auto_fix_high', 'history_edu', 'verified_user',
    'rocket_launch', 'auto_awesome', 'science', 'article'
  ];

  useEffect(() => {
    if (initialTab === 'general') {
      setActiveTab('history');
    } else {
      setActiveTab(initialTab);
    }
  }, [initialTab]);

  const loadHistory = async () => {
    try {
      const allTranscripts = await invoke<any[]>("get_transcripts");
      setTranscripts(allTranscripts);
    } catch (err) {
      console.error("Error loading history:", err);
    }
  };

  useEffect(() => {
    loadHistory();
    
    invoke<AudioDevice[]>("get_audio_devices")
      .then(setMicDevices)
      .catch(console.error);

    const unlisten = getCurrentWindow().listen("pipeline-results", () => {
      loadHistory();
    });

    return () => {
      unlisten.then(f => f());
    };
  }, []);

  const deleteTranscript = async (id: number) => {
    try {
      await invoke("delete_transcript", { id });
      loadHistory();
    } catch (err) {
      console.error("Error deleting transcript:", err);
    }
  };

  const clearHistory = async () => {
    if (confirm(t.confirm_clear)) {
      try {
        // Assume there's a clear_history command or just delete all IDs
        for (const t of transcripts) {
          await invoke("delete_transcript", { id: t.id });
        }
        loadHistory();
      } catch (err) {
        console.error("Error clearing history:", err);
      }
    }
  };

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
    <div className="h-full w-full flex items-center justify-center bg-background">
      <div className="text-on-surface-variant font-black tracking-[0.4em] text-[10px] animate-pulse uppercase">{t.loading}</div>
    </div>
  );

  const tabs = [
    { id: "history", label: t.history, icon: "schedule" },
    { id: "profiles", label: t.profiles, icon: "psychology" },
    { id: "dictionary", label: t.dictionary, icon: "book" },
    { id: "general", label: t.general, icon: "settings" },
  ];

  return (
    <div className="h-screen w-screen bg-background flex flex-col overflow-hidden select-none" onKeyDown={handleShortcutKeyDown} tabIndex={0}>
      <header className="flex items-center justify-between p-8 glass-panel">
        <div className="flex items-center gap-5">
          <div className="w-12 h-12 rounded-2xl cta-gradient flex items-center justify-center">
            <span className="material-symbols-outlined text-white font-black text-2xl material-symbols-fill">graphic_eq</span>
          </div>
          <div>
            <h1 className="text-xl font-bold text-on-surface tracking-widest font-headline">Voxa</h1>
            <div className="flex items-center gap-2.5 mt-0.5">
              <span className="w-1.5 h-1.5 rounded-full bg-primary animate-pulse" />
              <p className="font-label">{t.neural_engine}</p>
            </div>
          </div>
        </div>
        <button onClick={onClose} className="w-12 h-12 flex items-center justify-center bg-on-surface/5 hover:bg-on-surface/10 rounded-full transition-all text-on-surface-variant hover:text-on-surface group">
          <span className="material-symbols-outlined text-[24px] group-hover:rotate-90 transition-transform">close</span>
        </button>
      </header>

      <div className="flex-1 flex overflow-hidden">
        {/* Sidebar */}
        <aside className="w-80 p-8 space-y-2 bg-surface-container-low">
          <div className="mb-8 px-2">
            <p className="font-label opacity-40">{t.navigation}</p>
          </div>
          {tabs.map(tab => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`w-full flex items-center gap-5 px-6 py-4.5 rounded-voxa transition-all duration-300 text-sm font-bold group
                ${activeTab === tab.id 
                  ? 'bg-surface-container-high text-on-surface' 
                  : 'text-on-surface-variant hover:bg-surface-container-highest/30 hover:text-on-surface'}`}
            >
              <span className={`material-symbols-outlined text-[24px] ${activeTab === tab.id ? 'text-primary material-symbols-fill' : 'opacity-40 group-hover:opacity-100 group-hover:text-primary transition-all'}`}>
                {tab.icon}
              </span>
              <span className="tracking-tight">{tab.label}</span>
              {activeTab === tab.id && <div className="ml-auto w-1.5 h-1.5 rounded-full bg-primary" />}
            </button>
          ))}
          
          <div className="mt-auto pt-10">
             <div className="p-8 rounded-voxa bg-primary/5 border border-primary/10">
                <p className="text-[10px] text-on-surface-variant font-medium leading-relaxed italic pr-4">
                  {t.tip_text.split('Cmd+L').map((part, i, arr) => (
                    <span key={i}>
                      {part}
                      {i < arr.length - 1 && <span className="text-on-surface font-bold">Cmd+L</span>}
                    </span>
                  ))}
                </p>
             </div>
          </div>
        </aside>

        {/* Content */}
        <main ref={scrollContainerRef} className="flex-1 overflow-y-auto p-12 scroll-smooth bg-gradient-to-br from-transparent to-primary/[0.03] custom-scrollbar">
          <div className="max-w-3xl mx-auto space-y-12 animate-in fade-in slide-in-from-right-4 duration-500">
            
            {/* SECTION: HISTORY */}
            {activeTab === "history" && (
              <section className="space-y-8">
                <div className="flex items-center justify-between">
                  <div className="space-y-1">
                    <h3 className="text-xl font-black text-on-surface font-headline">{t.voice_history}</h3>
                    <p className="text-sm text-on-surface-variant">{t.history_subtitle}</p>
                  </div>
                  {transcripts.length > 0 && (
                    <button 
                      onClick={clearHistory}
                      className="px-4 py-2 rounded-xl bg-error/10 text-error text-[10px] font-black uppercase tracking-widest hover:bg-error/20 transition-all"
                    >
                      {t.clear_history}
                    </button>
                  )}
                </div>

                <div className="grid gap-6">
                  {transcripts.map((transcript) => (
                    <div key={transcript.id} className="group relative p-8 rounded-voxa bg-surface-container-low hover:bg-surface-container-high transition-all duration-500 shadow-lg">
                      <div className="flex justify-between items-start gap-4 mb-4">
                        <span className="text-[10px] font-mono text-on-surface-variant uppercase tracking-widest">
                          {new Date(transcript.timestamp).toLocaleDateString()} • {new Date(transcript.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                        </span>
                        <div className="flex gap-2 opacity-0 group-hover:opacity-100 transition-opacity">
                          <CopyButton text={transcript.content} copyLabel={t.copy_text} />
                          <button 
                            onClick={() => deleteTranscript(transcript.id)}
                            className="p-2 rounded-lg bg-error/10 text-error/40 hover:text-error hover:bg-error/20 transition-all"
                            title={t.delete}
                          >
                            <span className="material-symbols-outlined text-[18px]">delete</span>
                          </button>
                        </div>
                      </div>
                      <p className="text-sm text-on-surface leading-loose font-medium pr-12">
                        {transcript.content}
                      </p>
                    </div>
                  ))}
                  {transcripts.length === 0 && (
                    <div className="py-20 flex flex-col items-center justify-center space-y-4 border-2 border-dashed border-surface-container-high rounded-[3rem]">
                      <span className="material-symbols-outlined text-5xl text-on-surface-variant/10">history</span>
                      <p className="text-on-surface-variant/40 font-black uppercase tracking-[0.3em] text-[10px]">{t.no_transcripts}</p>
                    </div>
                  )}
                </div>
              </section>
            )}

            {/* SECTION: PROFILES */}
            {activeTab === "profiles" && (
              <section className="space-y-8">
                <div className="space-y-1">
                  <h3 className="text-xl font-black text-on-surface font-headline">{t.transformation_profiles}</h3>
                  <p className="text-sm text-on-surface-variant">{t.profiles_subtitle}</p>
                </div>

                <div className="grid gap-6">
                  {profiles.map(profile => (
                    <div key={profile.id} className="space-y-4">
                      <div
                        onClick={() => updateSetting("active_profile_id", profile.id.toString())}
                        className={`p-8 rounded-voxa transition-all text-left flex justify-between items-center group cursor-pointer
                          ${settings.active_profile_id === profile.id.toString() 
                            ? 'bg-surface-container-highest scale-[1.01]' 
                            : 'bg-surface-container-low hover:bg-surface-container-high shadow-lg'}`}
                      >
                        <div className="flex items-center gap-5 flex-1">
                          <div className={`w-14 h-14 rounded-2xl flex items-center justify-center transition-all ${settings.active_profile_id === profile.id.toString() ? 'bg-primary/20 text-primary' : 'bg-surface-container-highest text-on-surface-variant/40'}`}>
                             <span className="material-symbols-outlined text-[28px] material-symbols-fill">{profile.icon || 'psychology'}</span>
                          </div>
                          <div>
                            <div className="flex items-center gap-3">
                              <span className={`text-sm font-black uppercase tracking-widest ${settings.active_profile_id === profile.id.toString() ? 'text-on-surface' : 'text-on-surface-variant'}`}>
                                {profile.name}
                              </span>
                              {settings.active_profile_id === profile.id.toString() && (
                                <span className="px-2.5 py-1 rounded-full bg-primary/20 text-[9px] font-black text-primary">{t.active}</span>
                              )}
                            </div>
                            <div className="text-[11px] text-on-surface-variant/60 mt-1 line-clamp-1 italic max-w-md leading-relaxed">
                              "{profile.system_prompt || t.exact_transcription}"
                            </div>
                          </div>
                        </div>
                        
                        <div className="flex items-center gap-2">
                          <button 
                            onClick={(e) => {
                              e.stopPropagation();
                              if (editingProfileId === profile.id) {
                                setEditingProfileId(null);
                              } else {
                                setEditingProfileId(profile.id);
                                setEditName(profile.name);
                                setEditPrompt(profile.system_prompt);
                                setEditIcon(profile.icon || 'psychology');
                                setIsCreatingProfile(false);
                              }
                            }}
                            className={`p-3 rounded-xl transition-all ${editingProfileId === profile.id ? 'bg-on-surface text-background' : 'text-on-surface-variant/40 hover:text-on-surface hover:bg-surface-container-highest'}`}
                          >
                            <span className="material-symbols-outlined text-[20px]">edit</span>
                          </button>
                        </div>
                      </div>

                      {/* EDIT DRAWER */}
                      {editingProfileId === profile.id && (
                        <div className="mx-2 p-10 rounded-[3rem] bg-surface-container-high/80 backdrop-blur-3xl space-y-8 animate-in slide-in-from-top-4 duration-500 shadow-2xl ring-1 ring-white/10">
                           <div className="grid grid-cols-2 gap-6">
                             <div className="space-y-3">
                                <label className="text-[10px] font-black text-on-surface-variant uppercase tracking-widest ml-1">{t.name_label}</label>
                                <input 
                                  type="text" 
                                  value={editName}
                                  onChange={(e) => setEditName(e.target.value)}
                                  className="w-full bg-background/50 border border-surface-container-high p-4 rounded-2xl text-on-surface text-xs focus:outline-none focus:border-primary/40 transition-all font-bold"
                                />
                             </div>
                             <div className="space-y-3">
                                <label className="text-[10px] font-black text-on-surface-variant uppercase tracking-widest ml-1">{t.icon_label}</label>
                                <div className="flex flex-wrap gap-2.5 p-4 bg-primary/5 rounded-2xl max-h-[100px] overflow-y-auto custom-scrollbar ring-1 ring-primary/10">
                                  {AVAILABLE_ICONS.map(icon => (
                                    <button
                                      key={icon}
                                      onClick={() => setEditIcon(icon)}
                                      className={`p-2.5 rounded-xl transition-all ${editIcon === icon ? 'bg-primary text-on-primary shadow-lg shadow-primary/20' : 'text-on-surface-variant/40 hover:text-on-surface hover:bg-surface-container-high'}`}
                                    >
                                      <span className="material-symbols-outlined text-[22px]">{icon}</span>
                                    </button>
                                  ))}
                                </div>
                             </div>
                           </div>

                           <div className="space-y-3">
                              <label className="text-[10px] font-black text-on-surface-variant uppercase tracking-widest ml-1">{t.prompt_label}</label>
                              <textarea 
                                value={editPrompt}
                                onChange={(e) => setEditPrompt(e.target.value)}
                                rows={4}
                                className="w-full bg-background/50 border border-surface-container-high p-5 rounded-[2rem] text-on-surface text-xs focus:outline-none focus:border-primary/40 transition-all resize-none leading-relaxed font-semibold italic"
                                placeholder={t.prompt_placeholder}
                              />
                           </div>

                           <div className="flex gap-4 pt-2">
                              <button 
                                onClick={() => {
                                  updateProfile(profile.id, editName, editPrompt, editIcon);
                                  setEditingProfileId(null);
                                }}
                                className="flex-1 bg-on-surface text-background py-4 rounded-2xl text-[11px] font-black uppercase tracking-widest hover:bg-on-surface/90 active:scale-95 transition-all shadow-xl shadow-on-surface/5"
                              >
                                {t.save_profile}
                              </button>
                              
                              {profile.id > 4 && (
                                <button 
                                  onClick={() => {
                                    if (confirm(t.confirm_delete_profile.replace("{name}", profile.name))) {
                                      deleteProfile(profile.id);
                                    }
                                  }}
                                  className="px-8 bg-error/10 text-error py-4 rounded-2xl text-[11px] font-black uppercase tracking-widest hover:bg-error/20 active:scale-95 transition-all"
                                >
                                  {t.borrar}
                                </button>
                              )}

                              <button 
                                 onClick={() => setEditingProfileId(null)}
                                 className="px-10 bg-on-surface/5 text-on-surface/40 py-4 rounded-2xl text-[11px] font-black uppercase tracking-widest hover:bg-on-surface/10 transition-all"
                               >
                                 {t.cancel}
                               </button>
                           </div>
                        </div>
                      )}
                    </div>
                  ))}

                  {/* ADD NEW PROFILE BUTTON */}
                  {!isCreatingProfile ? (
                    <button
                      onClick={() => {
                        setIsCreatingProfile(true);
                        setEditingProfileId(null);
                        setNewName("");
                        setNewPrompt("");
                        setNewIcon("psychology");
                      }}
                      className="w-full p-8 rounded-[3rem] border-2 border-dashed border-surface-container-high text-on-surface-variant/40 hover:border-primary/40 hover:text-primary hover:bg-surface-container-low transition-all flex items-center justify-center gap-4 group"
                    >
                      <span className="material-symbols-outlined text-[24px] group-hover:scale-110 transition-transform">add_circle</span>
                      <span className="text-[11px] font-black uppercase tracking-[0.2em]">{t.create_new_profile}</span>
                    </button>
                  ) : (
                    <div className="p-8 rounded-[3rem] bg-primary/5 border border-primary/10 space-y-6 animate-in zoom-in-95 duration-300 shadow-2xl backdrop-blur-xl">
                      <div className="flex items-center gap-4 pb-4 border-b border-primary/5">
                        <div className="w-12 h-12 rounded-xl bg-primary/10 text-primary flex items-center justify-center">
                          <span className="material-symbols-outlined text-[24px] material-symbols-fill">{newIcon}</span>
                        </div>
                        <h4 className="text-xs font-black uppercase tracking-widest text-on-surface/60">{t.new_profile_title}</h4>
                      </div>

                      <div className="grid grid-cols-2 gap-6">
                        <div className="space-y-3">
                          <label className="text-[10px] font-black text-on-surface/20 uppercase tracking-widest ml-1">{t.name_label}</label>
                          <input 
                            type="text" 
                            placeholder={t.writer_example}
                            value={newName}
                            onChange={(e) => setNewName(e.target.value)}
                            className="w-full bg-on-surface/[0.03] border border-on-surface/5 p-4 rounded-2xl text-on-surface/80 text-xs focus:outline-none focus:border-on-surface/20 transition-all font-bold"
                          />
                        </div>
                        <div className="space-y-3">
                          <label className="text-[10px] font-black text-on-surface/20 uppercase tracking-widest ml-1">{t.icon_label}</label>
                          <div className="flex flex-wrap gap-2.5 p-3 bg-primary/5 rounded-2xl border border-primary/10 max-h-[100px] overflow-y-auto custom-scrollbar">
                            {AVAILABLE_ICONS.map(icon => (
                              <button
                                key={icon}
                                onClick={() => setNewIcon(icon)}
                                className={`p-2 rounded-xl transition-all ${newIcon === icon ? 'bg-primary text-on-primary shadow-lg shadow-primary/20' : 'text-on-surface/30 hover:text-on-surface hover:bg-on-surface/5'}`}
                              >
                                <span className="material-symbols-outlined text-[20px]">{icon}</span>
                              </button>
                            ))}
                          </div>
                        </div>
                      </div>

                        <div className="space-y-3">
                          <label className="text-[10px] font-black text-on-surface-variant uppercase tracking-widest ml-1 opacity-40">{t.instructions_custom}</label>
                          <textarea 
                            value={newPrompt}
                            onChange={(e) => setNewPrompt(e.target.value)}
                            rows={4}
                            className="w-full bg-background/40 p-6 rounded-[2rem] text-on-surface text-xs focus:outline-none ring-1 ring-on-surface/5 focus:ring-primary/20 transition-all resize-none leading-relaxed font-semibold italic"
                            placeholder={t.expert_example}
                          />
                        </div>

                      <div className="flex gap-4 pt-4">
                        <button 
                          onClick={() => {
                            if (newName && newPrompt) {
                              createProfile(newName, newPrompt, newIcon);
                              setIsCreatingProfile(false);
                            }
                          }}
                          className="flex-1 bg-on-surface text-background py-4 rounded-2xl text-[11px] font-black uppercase tracking-widest hover:bg-on-surface/90 active:scale-95 transition-all shadow-xl shadow-on-surface/5"
                        >
                          {t.create_profile}
                        </button>
                        <button 
                          onClick={() => setIsCreatingProfile(false)}
                          className="px-10 bg-on-surface/5 text-on-surface/40 py-4 rounded-2xl text-[11px] font-black uppercase tracking-widest hover:bg-on-surface/10 transition-all"
                        >
                          {t.discard}
                        </button>
                      </div>
                    </div>
                  )}
                </div>
              </section>
            )}

            {/* SECTION: DICTIONARY */}
            {activeTab === "dictionary" && (
              <section className="space-y-8">
                <div className="space-y-1">
                  <h3 className="text-xl font-black text-on-surface font-headline">{t.personal_dictionary}</h3>
                  <p className="text-sm text-on-surface-variant">{t.dictionary_subtitle}</p>
                </div>

                <div className="p-10 rounded-[3rem] bg-surface-container-low/40 space-y-10 group ring-1 ring-white/5">
                  <div className="flex flex-wrap gap-4">
                    {dictionary.map(word => (
                      <span 
                        key={word} 
                        className="bg-primary/5 px-6 py-3 rounded-2xl text-[11px] font-black tracking-widest text-primary flex items-center gap-3 hover:bg-primary/10 transition-all shadow-sm"
                      >
                        {word}
                        <button onClick={() => removeWord(word)} className="text-primary/30 hover:text-error transition-all">
                          <span className="material-symbols-outlined text-[18px]">close</span>
                        </button>
                      </span>
                    ))}
                    {dictionary.length === 0 && (
                      <div className="py-10 text-center w-full">
                        <p className="text-xs text-on-surface-variant/20 italic font-black uppercase tracking-[0.2em]">{t.dictionary_empty}</p>
                      </div>
                    )}
                  </div>
                  
                  <div className="flex gap-4 pt-10 bg-gradient-to-t from-transparent via-transparent to-on-surface/[0.02]">
                    <input 
                      type="text"
                      placeholder={t.dictionary_placeholder}
                      value={newWord}
                      onChange={(e) => setNewWord(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter' && newWord.trim()) {
                          addWord(newWord.trim());
                          setNewWord("");
                        }
                      }}
                      className="flex-1 bg-background/40 p-5 rounded-2xl text-on-surface text-sm focus:outline-none ring-1 ring-on-surface/5 focus:ring-primary/20 transition-all placeholder:text-on-surface-variant/20 font-bold"
                    />
                    <button 
                      onClick={() => { if (newWord.trim()) { addWord(newWord.trim()); setNewWord(""); } }}
                      className="bg-primary text-background px-10 rounded-2xl text-[11px] font-black uppercase tracking-widest hover:bg-primary-hover transition-all active:scale-95 shadow-xl shadow-primary/10"
                    >
                      {t.add}
                    </button>
                  </div>
                </div>
              </section>
            )}

            {/* SECTION: GENERAL */}
            {activeTab === "general" && (
              <section className="space-y-12">
                <div className="space-y-8">
                  <div className="space-y-1">
                    <h3 className="text-xl font-black text-on-surface font-headline">{t.system_settings}</h3>
                    <p className="text-sm text-on-surface-variant">{t.settings_subtitle}</p>
                  </div>

                  <div className="grid gap-8">
                    <div className="space-y-4">
                      <label className="text-[10px] font-black text-on-surface-variant uppercase tracking-[0.3em] ml-1">{t.audio_source}</label>
                      <div className="relative group">
                        <select 
                          value={settings.mic_id}
                          onChange={(e) => updateSetting("mic_id", e.target.value)}
                          className="w-full bg-surface-container-low p-6 rounded-voxa text-on-surface text-sm appearance-none focus:outline-none transition-all cursor-pointer hover:bg-surface-container-high font-bold"
                        >
                          <option value="auto">{t.auto_detect}</option>
                          {micDevices.map(dev => <option key={dev.id} value={dev.id}>{dev.name}</option>)}
                        </select>
                        <div className="absolute right-5 top-1/2 -translate-y-1/2 pointer-events-none text-on-surface-variant/40">
                          <span className="material-symbols-outlined">expand_more</span>
                        </div>
                      </div>
                    </div>

                    <div className="space-y-4">
                      <label className="text-[10px] font-black text-on-surface-variant uppercase tracking-[0.3em] ml-1">{t.global_shortcut}</label>
                      <button
                        onClick={() => setIsCapturingShortcut(true)}
                        className={`group w-full p-16 rounded-[4rem] transition-all flex flex-col items-center gap-6 relative overflow-hidden ring-1 shadow-2xl
                          ${isCapturingShortcut 
                             ? 'bg-primary/20 ring-primary/40 text-primary' 
                             : 'bg-surface-container-low/40 ring-on-surface/5 text-on-surface-variant/60 hover:bg-surface-container-high/60 hover:ring-primary/20'}`}
                      >
                        {isCapturingShortcut && <div className="absolute inset-0 bg-primary/10 animate-pulse" />}
                        <div className="text-[11px] font-black uppercase tracking-[0.5em] opacity-40 z-10">{t.selected_shortcut}</div>
                        <div className="text-6xl font-black tracking-tighter text-on-surface font-headline z-10">
                          {isCapturingShortcut ? t.listening : settings.global_shortcut.replace("Command", "⌘").replace("Alt", "⌥").replace("Shift", "⇧").replace("Control", "⌃")}
                        </div>
                        {!isCapturingShortcut && (
                          <div className="mt-8 px-10 py-4 rounded-2xl bg-on-surface/5 text-[10px] font-black uppercase tracking-widest group-hover:bg-primary group-hover:text-background transition-all z-10 shadow-lg">
                            {t.click_to_change}
                          </div>
                        )}
                      </button>
                    </div>

                    <div className="space-y-4">
                      <label className="text-[10px] font-black text-on-surface-variant uppercase tracking-[0.3em] ml-1">{t.transcription_input_lang}</label>
                      <div className="grid grid-cols-2 gap-4">
                        {['es', 'en'].map(lang => (
                          <button
                            key={lang}
                            onClick={() => updateSetting("language", lang)}
                            className={`py-8 rounded-[2rem] transition-all text-[12px] font-black uppercase tracking-[0.3em] ring-1
                              ${settings.language === lang 
                                ? 'bg-primary text-background shadow-[0_15px_40px_rgba(var(--color-primary-rgb),0.3)] ring-transparent scale-[1.03]' 
                                : 'bg-surface-container-low/40 ring-on-surface/5 text-on-surface-variant/40 hover:bg-surface-container-high/60 hover:text-on-surface hover:ring-on-surface/10'}`}
                          >
                            {lang === 'es' ? t.spanish : t.english}
                          </button>
                        ))}
                      </div>
                    </div>
                  </div>
                </div>
              </section>
            )}

          </div>
        </main>
      </div>

      <footer className="p-8 bg-surface-container-low/60 flex justify-between items-center">
        <div className="text-[9px] text-on-surface-variant/20 font-black uppercase tracking-[0.4em]">Engine v0.1.0 • macOS Native</div>
        <div className="flex gap-4">
           {/* Add more footer actions if needed */}
        </div>
      </footer>
    </div>
  );
}

function CopyButton({ text, copyLabel }: { text: string; copyLabel: string }) {
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <button 
      onClick={handleCopy}
      className={`p-2 rounded-lg transition-all ${copied ? 'bg-primary/10 text-primary' : 'bg-on-surface/5 text-on-surface/40 hover:text-on-surface hover:bg-on-surface/10'}`}
      title={copyLabel}
    >
      <span className="material-symbols-outlined text-[18px]">
        {copied ? 'done' : 'content_copy'}
      </span>
    </button>
  );
}

export default SettingsPanel;
