import { useState, useEffect, useRef } from "react";
import { useSettings, AppSettings } from "../hooks/useSettings";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Locale, translations } from "../i18n";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

interface SettingsPanelProps {
  initialTab?: string;
  uiLocale: Locale;
}

interface AudioDevice {
  id: string;
  name: string;
}

export function SettingsPanel({ initialTab = "general", uiLocale }: SettingsPanelProps) {
  const t = translations[uiLocale];
  const { settings, profiles, dictionaryEntries, updateSetting, addWord, removeWord, updateReplacement, updateProfile, createProfile, deleteProfile, loading } = useSettings();
  const [micDevices, setMicDevices] = useState<AudioDevice[]>([]);
  const [capturingShortcutFor, setCapturingShortcutFor] = useState<keyof AppSettings | null>(null);
  const capturingRef = useRef<keyof AppSettings | null>(null);
  const updateSettingRef = useRef(updateSetting);
  const [newWord, setNewWord] = useState("");
  const [appVersion, setAppVersion] = useState("1.0.0");
  const [activeTab, setActiveTab] = useState(initialTab === 'general' ? 'history' : initialTab);
  const [transcripts, setTranscripts] = useState<any[]>([]);
  const [editingTranscriptId, setEditingTranscriptId] = useState<number | null>(null);
  const [editingTranscriptText, setEditingTranscriptText] = useState("");
  const [learnedWords, setLearnedWords] = useState<string[]>([]);
  const [confirmModal, setConfirmModal] = useState<{ type: 'delete', id: number } | { type: 'delete-profile', id: number, name: string } | { type: 'clear' } | { type: 'redownload' } | null>(null);
  
  // State for models
  const [modelsInfo, setModelsInfo] = useState<any>(null);
  const [isDownloadingModels, setIsDownloadingModels] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState<any>(null);

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
    const fetchVersion = async () => {
      try {
        const version = await getVersion();
        setAppVersion(version);
      } catch (err) {
        console.error("Failed to fetch app version:", err);
      }
    };
    fetchVersion();
  }, []);

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

    const loadModelsInfo = async () => {
      try {
        const info = await invoke("get_models_info");
        setModelsInfo(info);
      } catch (err) {
        console.error("Failed to load models info:", err);
      }
    };
    loadModelsInfo();

    const unlistenProgress = getCurrentWindow().listen("download-progress", (event) => {
      setDownloadProgress(event.payload);
    });

    const unlisten = getCurrentWindow().listen("pipeline-results", () => {
      loadHistory();
    });

    return () => {
      unlisten.then(f => f());
      unlistenProgress.then(f => f());
    };
  }, []);

  const startEditTranscript = (id: number, content: string) => {
    setEditingTranscriptId(id);
    setEditingTranscriptText(content);
    setLearnedWords([]);
  };

  const saveTranscriptEdit = async (rawContent: string) => {
    if (editingTranscriptId === null) return;
    try {
      const learned = await invoke<string[]>("update_transcript", {
        id: editingTranscriptId,
        newContent: editingTranscriptText,
        rawContent,
      });
      setLearnedWords(learned);
      setTranscripts(prev => prev.map(t =>
        t.id === editingTranscriptId ? { ...t, content: editingTranscriptText } : t
      ));
      if (learned.length === 0) setEditingTranscriptId(null);
    } catch (err) {
      console.error("Error saving transcript:", err);
    }
  };

  const deleteTranscript = (id: number) => {
    setConfirmModal({ type: 'delete', id });
  };

  const executeDelete = async (id: number) => {
    setConfirmModal(null);
    try {
      await invoke("delete_transcript", { id });
      loadHistory();
    } catch (err) {
      console.error("Error deleting transcript:", err);
    }
  };

  const clearHistory = () => {
    setConfirmModal({ type: 'clear' });
  };

  const executeRedownload = async () => {
    setConfirmModal(null);
    setIsDownloadingModels(true);
    setDownloadProgress(null);
    try {
      await invoke("download_models");
      const info = await invoke("get_models_info");
      setModelsInfo(info);
    } catch (e) {
      console.error(e);
    }
    setIsDownloadingModels(false);
    setDownloadProgress(null);
  };

  const executeClearHistory = async () => {
    setConfirmModal(null);
    try {
      await invoke("clear_transcripts");
      loadHistory();
    } catch (err) {
      console.error("Error clearing history:", err);
    }
  };


  // Keep refs up to date on every render so the keydown listener always reads fresh values
  capturingRef.current = capturingShortcutFor;
  updateSettingRef.current = updateSetting;

  // Unregister Tauri shortcuts when entering capture mode, restore when leaving.
  useEffect(() => {
    if (!capturingShortcutFor) return;

    let isCancelled = false;

    const performCapture = async () => {
      try {
        console.log("--- STARTING NATIVE CAPTURE ---");
        await invoke("unregister_all_shortcuts");
        const result = await invoke<string>("start_native_key_capture");
        
        if (isCancelled) {
          console.log("Capture component unmounted, ignoring result.");
          return;
        }

        console.log("Native capture result:", result);
        // Special case: Escape to cancel, or if it's Just a modifier + something we can't map
        if (result === "Escape") {
          setCapturingShortcutFor(null);
        } else if (result) {
          updateSetting(capturingShortcutFor, result);
          setCapturingShortcutFor(null);
        }
      } catch (e) {
        console.error("Native Capture Error:", e);
        // Fallback or just stop
        setCapturingShortcutFor(null);
      } finally {
        if (!isCancelled) {
          await invoke("apply_all_shortcuts").catch(console.error);
        }
      }
    };

    performCapture();

    return () => {
      isCancelled = true;
    };
  }, [capturingShortcutFor, updateSetting]);

  // Clean up: We no longer need the global window keydown listener for shortcut capture 
  // since we use the native macOS NSEvent monitoring to avoid the "bonk" sound.
  useEffect(() => {
    // If we want to support non-macOS later, we could re-add a conditional listener here.
    // For now, on Mac, the native capture is the only source of truth.
  }, []);

  if (loading || !settings) return (
    <div className="h-full w-full flex items-center justify-center bg-background">
      <div className="text-on-surface-variant font-black tracking-[0.4em] text-[10px] animate-pulse uppercase">{t.loading}</div>
    </div>
  );

  const tabs = [
    { id: "history", label: t.history, icon: "schedule" },
    { id: "profiles", label: t.profiles, icon: "psychology" },
    { id: "dictionary", label: t.dictionary, icon: "book" },
    { id: "models", label: t.models, icon: "memory" },
    { id: "general", label: t.general, icon: "settings" },
  ];

  return (
    <div className="h-screen w-screen bg-background flex flex-col overflow-hidden select-none">
      <header className="flex items-center justify-between p-8 glass-panel">
        <div className="flex items-center gap-6">
          <div className="w-14 h-14 rounded-[1.25rem] bg-primary flex items-center justify-center shadow-lg shadow-primary/20">
            <svg width="32" height="32" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
              <rect x="1.5" y="7" width="3" height="10" rx="1.5" fill="white" />
              <rect x="6" y="4" width="3" height="16" rx="1.5" fill="white" />
              <rect x="10.5" y="1" width="3" height="22" rx="1.5" fill="white" />
              <rect x="15" y="4" width="3" height="16" rx="1.5" fill="white" />
              <rect x="19.5" y="7" width="3" height="10" rx="1.5" fill="white" />
            </svg>
          </div>
          <div>
            <h1 className="text-2xl font-black text-on-surface font-headline leading-none">Voxa</h1>
            <div className="flex items-center gap-2 mt-2">
              <span className="w-2 h-2 rounded-full bg-primary/40" />
              <p className="text-[10px] font-black text-on-surface-variant uppercase tracking-[0.15em] opacity-70 leading-none">{t.app_subtitle}</p>
            </div>
          </div>
        </div>
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
                  {transcripts.map((transcript) => {
                    const isEditing = editingTranscriptId === transcript.id;
                    return (
                    <div key={transcript.id} className="group relative p-8 rounded-voxa bg-surface-container-low hover:bg-surface-container-high transition-all duration-500 shadow-lg">
                      <div className="flex justify-between items-start gap-4 mb-4">
                        <span className="text-[10px] font-mono text-on-surface-variant uppercase tracking-widest">
                          {new Date(transcript.timestamp).toLocaleDateString()} • {new Date(transcript.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                        </span>
                        <div className="flex gap-2 opacity-0 group-hover:opacity-100 transition-opacity">
                          {!isEditing && <CopyButton text={transcript.content} copyLabel={t.copy_text} />}
                          {!isEditing && (
                            <button
                              onClick={() => startEditTranscript(transcript.id, transcript.content)}
                              className="p-2 rounded-lg bg-primary/10 text-primary/40 hover:text-primary hover:bg-primary/20 transition-all"
                              title={t.edit ?? "Edit"}
                            >
                              <span className="material-symbols-outlined text-[18px]">edit</span>
                            </button>
                          )}
                          {!isEditing && (
                            <button
                              onClick={() => deleteTranscript(transcript.id)}
                              className="p-2 rounded-lg bg-error/10 text-error/40 hover:text-error hover:bg-error/20 transition-all"
                              title={t.delete}
                            >
                              <span className="material-symbols-outlined text-[18px]">delete</span>
                            </button>
                          )}
                        </div>
                      </div>

                      {isEditing ? (
                        <div className="space-y-4">
                          <textarea
                            className="w-full text-sm text-on-surface leading-loose font-medium bg-surface-container rounded-xl p-4 border border-primary/20 focus:border-primary outline-none resize-none"
                            rows={4}
                            value={editingTranscriptText}
                            onChange={e => setEditingTranscriptText(e.target.value)}
                            autoFocus
                          />
                          {learnedWords.length > 0 && (
                            <div className="flex flex-wrap gap-2">
                              <span className="text-[10px] font-black uppercase tracking-widest text-primary/60">{t.learned_words ?? "Learned"}:</span>
                              {learnedWords.map(w => (
                                <span key={w} className="text-[10px] font-mono bg-primary/10 text-primary px-2 py-0.5 rounded-full">{w}</span>
                              ))}
                            </div>
                          )}
                          <div className="flex gap-3">
                            <button
                              onClick={() => saveTranscriptEdit(transcript.raw_content)}
                              className="flex-1 bg-primary text-white py-3 rounded-xl text-[11px] font-black uppercase tracking-widest hover:bg-primary/90 active:scale-95 transition-all"
                            >
                              {t.save_profile ?? "Save"}
                            </button>
                            <button
                              onClick={() => { setEditingTranscriptId(null); setLearnedWords([]); }}
                              className="px-6 bg-on-surface/5 text-on-surface/40 py-3 rounded-xl text-[11px] font-black uppercase tracking-widest hover:bg-on-surface/10 transition-all"
                            >
                              {t.cancel}
                            </button>
                          </div>
                        </div>
                      ) : (
                        <p className="text-sm text-on-surface leading-loose font-medium pr-12">
                          {transcript.content}
                        </p>
                      )}
                    </div>
                    );
                  })}
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
                              
                              {!profile.is_default && (
                                <button
                                  onClick={() => setConfirmModal({ type: 'delete-profile', id: profile.id, name: profile.name })}
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
                  {dictionaryEntries.length > 0 ? (
                    <div className="overflow-x-auto">
                      <table className="w-full text-sm">
                        <thead>
                          <tr className="text-[10px] font-black uppercase tracking-widest text-on-surface-variant/40 border-b border-on-surface/5">
                            <th className="text-left py-3 pr-4">{t.word ?? "Word"}</th>
                            <th className="text-left py-3 pr-4">{t.replacement ?? "Replacement"}</th>
                            <th className="text-right py-3 pr-4 w-20">{t.usage ?? "Uses"}</th>
                            <th className="w-8"></th>
                          </tr>
                        </thead>
                        <tbody>
                          {dictionaryEntries.map(entry => (
                            <tr key={entry.word} className="border-b border-on-surface/5 last:border-0 group/row">
                              <td className="py-3 pr-4 font-bold text-on-surface tracking-wide">{entry.word}</td>
                              <td className="py-3 pr-4">
                                <input
                                  type="text"
                                  placeholder={t.replacement_placeholder ?? "e.g. correct spelling"}
                                  defaultValue={entry.replacement_word ?? ""}
                                  onBlur={(e) => {
                                    const val = e.target.value.trim() || null;
                                    updateReplacement(entry.word, val);
                                  }}
                                  onKeyDown={(e) => {
                                    if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
                                  }}
                                  className="bg-background/40 px-3 py-1.5 rounded-xl text-on-surface text-xs focus:outline-none ring-1 ring-on-surface/5 focus:ring-primary/20 transition-all placeholder:text-on-surface-variant/20 font-medium w-full"
                                />
                              </td>
                              <td className="py-3 pr-4 text-right">
                                {entry.usage_count > 0 ? (
                                  <span className="bg-primary/10 text-primary text-[10px] font-black px-2.5 py-1 rounded-full">
                                    {entry.usage_count}
                                  </span>
                                ) : (
                                  <span className="text-on-surface-variant/20 text-[10px] font-black">—</span>
                                )}
                              </td>
                              <td className="py-3 text-right">
                                <button onClick={() => removeWord(entry.word)} className="text-on-surface-variant/20 hover:text-error transition-all opacity-0 group-hover/row:opacity-100">
                                  <span className="material-symbols-outlined text-[18px]">close</span>
                                </button>
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  ) : (
                    <div className="py-10 text-center w-full">
                      <p className="text-xs text-on-surface-variant/20 italic font-black uppercase tracking-[0.2em]">{t.dictionary_empty}</p>
                    </div>
                  )}

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

            {/* SECTION: MODELS */}
            {activeTab === "models" && (
              <section className="space-y-8">
                <div className="flex items-center justify-between">
                  <div className="space-y-1">
                    <h3 className="text-xl font-black text-on-surface font-headline">{t.models}</h3>
                    <p className="text-sm text-on-surface-variant">{t.models_subtitle}</p>
                  </div>
                  <button 
                    onClick={() => invoke("open_models_folder")}
                    className="px-6 py-3 rounded-xl bg-primary/5 text-primary text-[10px] font-black uppercase tracking-widest hover:bg-primary/10 transition-all flex items-center gap-2"
                  >
                    <span className="material-symbols-outlined text-[16px]">folder_open</span>
                    {t.open_folder}
                  </button>
                </div>

                {modelsInfo && (
                  <div className="space-y-6">
                    <div className="p-8 rounded-[2rem] bg-surface-container-low shadow-lg space-y-6">
                      <div className="space-y-2">
                        <label className="text-[10px] font-black text-on-surface-variant uppercase tracking-widest ml-1">{t.path}</label>
                        <div className="text-[11px] text-on-surface-variant/80 font-mono break-all bg-background border border-surface-container-high p-4 rounded-xl">
                          {modelsInfo.base_path}
                        </div>
                      </div>

                      <div className="space-y-4 pt-6 border-t border-on-surface/5">
                        <label className="text-[10px] font-black text-on-surface-variant uppercase tracking-widest ml-1">{t.ai_models}</label>
                        <div className="grid gap-4">
                          {modelsInfo.models.map((model: any) => (
                            <div key={model.filename} className="flex justify-between items-center p-6 rounded-2xl bg-background border border-surface-container-high transition-all">
                              <div>
                                <h4 className="text-sm font-black text-on-surface uppercase tracking-widest">{model.display_name}</h4>
                                <div className="text-[10px] text-on-surface-variant/60 font-mono mt-1 opacity-60">{model.filename}</div>
                              </div>
                              <div className="text-right">
                                <div className="text-[11px] font-black text-on-surface uppercase tracking-widest">{model.size_mb} {t.size_mb}</div>
                                <div className={`text-[9px] font-black uppercase tracking-[0.2em] mt-1.5 px-2 py-1 inline-block rounded-md ${model.downloaded ? 'bg-primary/10 text-primary' : 'bg-error/10 text-error'}`}>
                                  {model.downloaded ? t.downloaded : t.missing}
                                </div>
                              </div>
                            </div>
                          ))}
                        </div>
                      </div>
                    </div>

                    <div className="flex justify-end pt-2">
                      <button
                        onClick={() => setConfirmModal({ type: 'redownload' })}
                        disabled={isDownloadingModels}
                        className={`px-8 py-4 rounded-2xl text-[11px] font-black uppercase tracking-widest transition-all ${isDownloadingModels ? 'bg-surface-container-highest text-on-surface-variant/40 cursor-not-allowed' : 'bg-on-surface text-background hover:bg-on-surface/90 shadow-xl shadow-on-surface/5'}`}
                      >
                        {isDownloadingModels ? t.loading : t.redownload}
                      </button>
                    </div>
                    {isDownloadingModels && downloadProgress && (
                      <div className="p-8 rounded-[2rem] bg-surface-container-low border border-surface-container-high space-y-4 animate-in slide-in-from-bottom-4 duration-500 shadow-xl">
                         <div className="flex justify-between text-[10px] font-black uppercase tracking-widest text-primary">
                            <span>{downloadProgress.model}</span>
                            <span>{downloadProgress.progress.toFixed(1)}%</span>
                         </div>
                         <div className="w-full h-2 bg-on-surface/5 rounded-full overflow-hidden">
                            <div className="h-full bg-primary transition-all duration-300 shadow-[0_0_10px_rgba(var(--color-primary-rgb),0.5)]" style={{ width: `${downloadProgress.progress}%` }} />
                         </div>
                      </div>
                    )}
                  </div>
                )}
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

                  <div className="space-y-6">
                    {/* Audio Configuration */}
                    <div className="p-8 rounded-[2rem] bg-surface-container-low/50 border border-surface-container-high transition-all hover:bg-surface-container-low hover:shadow-lg group/card">
                      <div className="flex items-center gap-4 mb-6">
                         <div className="w-12 h-12 rounded-2xl bg-primary/10 flex items-center justify-center text-primary group-hover/card:scale-105 transition-transform">
                           <span className="material-symbols-outlined text-[24px]">mic</span>
                         </div>
                         <div>
                           <h4 className="text-sm font-black text-on-surface uppercase tracking-widest">{t.audio_source}</h4>
                           {/* Descripciones en duro eliminadas para mantener minimalismo y consistencia i18n */}
                         </div>
                      </div>
                      <div className="relative group pl-[64px]">
                        <select 
                          value={settings.mic_id}
                          onChange={(e) => updateSetting("mic_id", e.target.value)}
                          className="w-full bg-background border border-surface-container-high p-5 rounded-2xl text-on-surface text-sm appearance-none focus:outline-none focus:border-primary/40 transition-all cursor-pointer hover:bg-surface-container-highest font-bold"
                        >
                          <option value="auto">{t.auto_detect}</option>
                          {micDevices.map(dev => <option key={dev.id} value={dev.id}>{dev.name}</option>)}
                        </select>
                        <div className="absolute right-5 top-1/2 -translate-y-1/2 pointer-events-none text-on-surface-variant/40">
                          <span className="material-symbols-outlined">expand_more</span>
                        </div>
                      </div>
                    </div>

                    {/* Shortcuts Configuration */}
                    <div className="p-8 rounded-[2rem] bg-surface-container-low/50 border border-surface-container-high transition-all hover:bg-surface-container-low hover:shadow-lg group/card">
                      <div className="flex items-center justify-between mb-6">
                        <div className="flex items-center gap-4">
                           <div className="w-12 h-12 rounded-2xl bg-primary/10 flex items-center justify-center text-primary group-hover/card:scale-105 transition-transform">
                             <span className="material-symbols-outlined text-[24px]">keyboard</span>
                           </div>
                           <div>
                             <h4 className="text-sm font-black text-on-surface uppercase tracking-widest">{t.global_shortcut}</h4>
                           </div>
                        </div>
                        <button 
                          onClick={() => {
                            updateSetting("shortcut_push_to_talk", "Alt+Space");
                            updateSetting("shortcut_hands_free", "F5");
                            updateSetting("shortcut_paste", "CommandOrControl+Shift+V");
                            updateSetting("shortcut_cancel", "Escape");
                          }}
                          className="px-4 py-2 rounded-xl bg-on-surface/5 text-on-surface-variant font-black text-[10px] uppercase tracking-widest hover:bg-on-surface/10 transition-all"
                        >
                          {t.reset_defaults}
                        </button>
                      </div>
                      
                      <div className="pl-[64px] grid grid-cols-2 gap-4">
                        {[
                          { key: "shortcut_push_to_talk", label: t.shortcut_push_to_talk },
                          { key: "shortcut_hands_free", label: t.shortcut_hands_free },
                          { key: "shortcut_paste", label: t.shortcut_paste },
                          { key: "shortcut_cancel", label: t.shortcut_cancel }
                        ].map((sc) => {
                          const isCapturing = capturingShortcutFor === sc.key;
                          const currentVal = settings[sc.key as keyof AppSettings] as string || "";
                          const displayVal = isCapturing 
                             ? t.listening 
                             : currentVal.replace("CommandOrControl", "⌘").replace("Alt", "⌥").replace("Shift", "⇧").replace("Escape", "Esc");
                             
                          return (
                            <button
                              key={sc.key}
                              onClick={() => setCapturingShortcutFor(sc.key as keyof AppSettings)}
                              className={`group p-6 rounded-[1.5rem] transition-all flex flex-col items-start gap-2 relative overflow-hidden ring-1 shadow-sm
                                ${isCapturing 
                                   ? 'bg-primary/20 ring-primary/40 text-primary scale-[1.02]' 
                                   : 'bg-background ring-surface-container-high text-on-surface hover:bg-surface-container-highest hover:ring-primary/20'}`}
                            >
                              {isCapturing && <div className="absolute inset-0 bg-primary/5 animate-pulse" />}
                              <div className={`text-[10px] font-black uppercase tracking-widest z-10 ${isCapturing ? 'text-primary' : 'text-on-surface-variant'}`}>{sc.label}</div>
                              <div className="text-xl font-black tracking-tighter z-10 flex items-center justify-between w-full">
                                <span>{displayVal}</span>
                                {!isCapturing && <span className="material-symbols-outlined text-[14px] opacity-0 group-hover:opacity-100 transition-opacity">edit</span>}
                              </div>
                            </button>
                          );
                        })}
                      </div>
                    </div>

                    {/* Auto-detect profile */}
                    <div className="p-8 rounded-[2rem] bg-surface-container-low/50 border border-surface-container-high transition-all hover:bg-surface-container-low hover:shadow-lg group/card">
                      <div className="flex items-center justify-between">
                        <div className="flex items-center gap-4">
                          <div className="w-12 h-12 rounded-2xl bg-primary/10 flex items-center justify-center text-primary group-hover/card:scale-105 transition-transform">
                            <span className="material-symbols-outlined text-[24px]">auto_awesome</span>
                          </div>
                          <div>
                            <h4 className="text-sm font-black text-on-surface uppercase tracking-widest">{t.auto_detect_profile}</h4>
                            <p className="text-xs text-on-surface-variant mt-1 max-w-sm">{t.auto_detect_profile_hint}</p>
                          </div>
                        </div>
                        <button
                          onClick={() => {
                            const next = settings.auto_detect_profile !== "true" ? "true" : "false";
                            updateSetting("auto_detect_profile" as keyof AppSettings, next);
                          }}
                          className={`relative w-14 h-8 rounded-full transition-colors flex-shrink-0 ${
                            settings.auto_detect_profile !== "false" ? "bg-primary" : "bg-surface-container-high"
                          }`}
                        >
                          <span className={`absolute top-1 w-6 h-6 rounded-full bg-white shadow transition-transform ${
                            settings.auto_detect_profile !== "false" ? "translate-x-7" : "translate-x-1"
                          }`} />
                        </button>
                      </div>
                    </div>

                    {/* Language Configuration */}
                    <div className="p-8 rounded-[2rem] bg-surface-container-low/50 border border-surface-container-high transition-all hover:bg-surface-container-low hover:shadow-lg group/card">
                      <div className="flex items-center gap-4 mb-6">
                         <div className="w-12 h-12 rounded-2xl bg-primary/10 flex items-center justify-center text-primary group-hover/card:scale-105 transition-transform">
                           <span className="material-symbols-outlined text-[24px]">translate</span>
                         </div>
                         <div>
                           <h4 className="text-sm font-black text-on-surface uppercase tracking-widest">{t.transcription_input_lang}</h4>
                         </div>
                      </div>
                      <div className="pl-[64px] grid grid-cols-2 gap-4">
                        {['es', 'en'].map(lang => (
                          <button
                            key={lang}
                            onClick={() => updateSetting("language", lang)}
                            className={`py-8 rounded-[2rem] transition-all text-[11px] font-black uppercase tracking-[0.3em] ring-1
                              ${settings.language === lang 
                                ? 'bg-primary text-background shadow-lg ring-transparent scale-[1.02]' 
                                : 'bg-background ring-surface-container-high text-on-surface-variant/40 hover:bg-surface-container-highest hover:text-on-surface hover:ring-on-surface/10'}`}
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
        <div className="text-[9px] text-on-surface-variant/60 font-black tracking-widest">
          {t.footer_engine.replace("{version}", appVersion)}
        </div>
        <div className="flex gap-4">
           {/* Add more footer actions if needed */}
        </div>
      </footer>

      {/* Confirmation Modal */}
      {confirmModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-on-surface/20 backdrop-blur-sm">
          <div className="bg-surface rounded-2xl shadow-2xl p-8 max-w-sm w-full mx-6 space-y-6">
            <div className="space-y-2">
              <h3 className="text-base font-black text-on-surface font-headline uppercase tracking-widest">
                {confirmModal.type === 'redownload' ? t.redownload : t.delete}
              </h3>
              <p className="text-sm text-on-surface-variant">
                {confirmModal.type === 'clear' ? t.confirm_clear
                  : confirmModal.type === 'redownload' ? t.redownload + "?"
                  : confirmModal.type === 'delete-profile' ? t.confirm_delete_profile.replace("{name}", confirmModal.name)
                  : t.confirm_delete_transcript}
              </p>
            </div>
            <div className="flex gap-3 justify-end">
              <button
                onClick={() => setConfirmModal(null)}
                className="px-5 py-2 rounded-xl bg-surface-container text-on-surface-variant text-[11px] font-black uppercase tracking-widest hover:bg-surface-container-high transition-all"
              >
                {t.cancel}
              </button>
              <button
                onClick={() => {
                  if (confirmModal.type === 'redownload') executeRedownload();
                  else if (confirmModal.type === 'clear') executeClearHistory();
                  else if (confirmModal.type === 'delete-profile') { deleteProfile(confirmModal.id); setConfirmModal(null); setEditingProfileId(null); }
                  else executeDelete(confirmModal.id);
                }}
                className="px-5 py-2 rounded-xl bg-error text-white text-[11px] font-black uppercase tracking-widest hover:bg-error/90 transition-all"
              >
                {confirmModal.type === 'redownload' ? t.redownload : t.delete}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function CopyButton({ text, copyLabel }: { text: string; copyLabel: string }) {
  const [copied, setCopied] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => { if (timerRef.current) clearTimeout(timerRef.current); };
  }, []);

  const handleCopy = async () => {
    await writeText(text);
    setCopied(true);
    timerRef.current = setTimeout(() => setCopied(false), 2000);
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
