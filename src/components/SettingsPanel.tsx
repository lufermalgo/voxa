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
  const { settings, profiles, dictionaryEntries, updateSetting, addWord, removeWord, updateReplacement, updateProfile, updateProfileFormattingMode, createProfile, deleteProfile, loading } = useSettings();
  const [micDevices, setMicDevices] = useState<AudioDevice[]>([]);
  const [capturingShortcutFor, setCapturingShortcutFor] = useState<keyof AppSettings | null>(null);
  const capturingRef = useRef<keyof AppSettings | null>(null);
  const updateSettingRef = useRef(updateSetting);
  const [newWord, setNewWord] = useState("");
  const [appVersion, setAppVersion] = useState("1.0.0");
  const [activeTab, setActiveTab] = useState(initialTab === 'general' ? 'history' : initialTab);
  const [expandedTranscriptId, setExpandedTranscriptId] = useState<number | null>(null);
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
      {/* ── HEADER ── 48px compacto */}
      <header className="h-12 flex items-center justify-between px-8 border-b border-on-surface/[0.10] flex-shrink-0 bg-surface-container-low/30">
        <div className="flex items-center gap-3">
          <div className="w-9 h-9 rounded-[0.6rem] bg-primary flex items-center justify-center shadow-lg shadow-primary/40 ring-1 ring-primary/30 flex-shrink-0">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
              <rect x="1.5" y="7" width="3" height="10" rx="1.5" fill="white" />
              <rect x="6" y="4" width="3" height="16" rx="1.5" fill="white" />
              <rect x="10.5" y="1" width="3" height="22" rx="1.5" fill="white" />
              <rect x="15" y="4" width="3" height="16" rx="1.5" fill="white" />
              <rect x="19.5" y="7" width="3" height="10" rx="1.5" fill="white" />
            </svg>
          </div>
          <div className="flex items-baseline gap-2">
            <span className="text-sm font-black text-on-surface font-headline leading-none tracking-tight">Voxa</span>
            <span className="text-[9px] font-semibold text-on-surface-variant/80 uppercase tracking-[0.2em] hidden sm:inline">{t.app_subtitle}</span>
          </div>
        </div>
        <span className="text-[9px] font-mono text-on-surface-variant/70 tracking-widest">v{appVersion}</span>
      </header>

      <div className="flex-1 flex overflow-hidden">
        {/* ── SIDEBAR ── 200px, barra izquierda activa */}
        <aside className="w-[200px] flex flex-col py-6 border-r border-on-surface/[0.10] flex-shrink-0 bg-surface-container-low/30">
          <nav className="flex-1 px-3 space-y-0.5">
            {tabs.map(tab => (
              <button
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                className={`relative w-full flex items-center gap-3 px-3 py-2 rounded-lg transition-colors duration-150 text-xs font-semibold
                  ${activeTab === tab.id
                    ? 'text-on-surface font-bold bg-primary/[0.08]'
                    : 'text-on-surface-variant hover:text-on-surface hover:bg-surface-container-low/70'}`}
              >
                {activeTab === tab.id && (
                  <span className="absolute left-0 top-1/2 -translate-y-1/2 w-0.5 h-4 bg-primary rounded-full" />
                )}
                <span className={`material-symbols-outlined text-[18px] ${activeTab === tab.id ? 'material-symbols-fill text-primary' : 'text-on-surface-variant/70'}`}>
                  {tab.icon}
                </span>
                <span className="tracking-tight">{tab.label}</span>
              </button>
            ))}
          </nav>
          <div className="px-4 pb-2">
            <p className="text-[9px] text-on-surface-variant/80 leading-relaxed">
              <span className="text-on-surface-variant font-bold">Cmd+L</span>{' '}
              {t.tip_text.replace(/.*Cmd\+L\s*/i, '')}
            </p>
          </div>
        </aside>

        {/* ── CONTENT AREA ── px-10 py-8, sin max-w */}
        <main ref={scrollContainerRef} className="flex-1 overflow-y-auto px-10 py-8 scroll-smooth custom-scrollbar">
          <div className="animate-in fade-in slide-in-from-right-1 duration-200">

            {/* ── HISTORY ── feed tipo log agrupado por fecha */}
            {activeTab === "history" && (
              <section>
                <div className="flex items-center justify-between mb-6">
                  <div className="flex items-baseline gap-2">
                    <h3 className="text-xl font-black text-on-surface font-headline">{t.voice_history}</h3>
                    {transcripts.length > 0 && (
                      <span className="text-[10px] text-on-surface-variant font-mono">({transcripts.length})</span>
                    )}
                  </div>
                  {transcripts.length > 0 && (
                    <button onClick={clearHistory} className="text-[9px] font-semibold uppercase tracking-widest text-error hover:text-error/80 transition-colors py-1">
                      {t.clear_history}
                    </button>
                  )}
                </div>

                {transcripts.length === 0 ? (
                  <div className="py-16 flex flex-col items-center gap-3">
                    <span className="material-symbols-outlined text-4xl text-on-surface-variant/[0.06]">history</span>
                    <p className="text-[10px] font-black uppercase tracking-[0.3em] text-on-surface-variant/80">{t.no_transcripts}</p>
                  </div>
                ) : (
                  <div>
                    {(() => {
                      const groups: { label: string; items: typeof transcripts }[] = [];
                      const today = new Date(); today.setHours(0,0,0,0);
                      const yesterday = new Date(today); yesterday.setDate(yesterday.getDate() - 1);
                      transcripts.forEach(tr => {
                        const d = new Date(tr.timestamp); d.setHours(0,0,0,0);
                        let label: string;
                        if (d.getTime() === today.getTime()) label = t.history || 'Today';
                        else if (d.getTime() === yesterday.getTime()) label = 'Yesterday';
                        else label = d.toLocaleDateString();
                        const existing = groups.find(g => g.label === label);
                        if (existing) existing.items.push(tr);
                        else groups.push({ label, items: [tr] });
                      });
                      return groups.map((group, gi) => (
                        <div key={group.label} className={gi > 0 ? 'mt-6' : ''}>
                          <div className="flex items-center justify-between py-2 border-t border-on-surface/[0.10]">
                            <span className="text-[9px] font-bold uppercase tracking-[0.3em] text-on-surface-variant">{group.label}</span>
                          </div>
                          {group.items.map((transcript) => {
                            const isEditing = editingTranscriptId === transcript.id;
                            const isExpanded = expandedTranscriptId === transcript.id;
                            const time = new Date(transcript.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
                            return (
                              <div key={transcript.id}>
                                {isEditing ? (
                                  <div className="py-3 border-b border-on-surface/[0.08] space-y-3">
                                    <textarea
                                      className="w-full text-sm text-on-surface leading-relaxed font-medium bg-surface-container rounded-lg px-3 py-2 focus:outline-none focus:ring-1 focus:ring-primary/30 resize-none"
                                      rows={3} value={editingTranscriptText}
                                      onChange={e => setEditingTranscriptText(e.target.value)} autoFocus
                                    />
                                    {learnedWords.length > 0 && (
                                      <div className="flex flex-wrap gap-1.5">
                                        <span className="text-[9px] font-black uppercase tracking-widest text-primary/90">{t.learned_words ?? "Learned"}:</span>
                                        {learnedWords.map(w => <span key={w} className="text-[9px] font-mono bg-primary/10 text-primary px-2 py-0.5 rounded-full">{w}</span>)}
                                      </div>
                                    )}
                                    <div className="flex gap-2">
                                      <button onClick={() => saveTranscriptEdit(transcript.raw_content)} className="flex-1 bg-primary text-background py-2 rounded-lg text-[10px] font-black uppercase tracking-wider hover:bg-primary/90 transition-all">{t.save_profile ?? "Save"}</button>
                                      <button onClick={() => { setEditingTranscriptId(null); setLearnedWords([]); }} className="px-4 bg-surface-container text-on-surface-variant py-2 rounded-lg text-[10px] font-black uppercase tracking-wider hover:bg-surface-container-high transition-all">{t.cancel}</button>
                                    </div>
                                  </div>
                                ) : (
                                  <div className="group flex items-center gap-3 py-2.5 border-b border-on-surface/[0.08] last:border-0 hover:bg-surface-container transition-colors rounded-sm -mx-2 px-2">
                                    <span className="font-mono text-[10px] text-on-surface-variant/80 flex-shrink-0 w-10">{time}</span>
                                    <p className={`flex-1 text-sm text-on-surface font-medium min-w-0 cursor-pointer ${isExpanded ? 'whitespace-pre-wrap' : 'truncate'}`} onClick={() => setExpandedTranscriptId(isExpanded ? null : transcript.id)}>{transcript.content}</p>
                                    <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity flex-shrink-0">
                                      <CopyButton text={transcript.content} copyLabel={t.copy_text} />
                                      <button onClick={() => startEditTranscript(transcript.id, transcript.content)} className="p-1 rounded text-on-surface-variant/80 hover:text-primary transition-colors"><span className="material-symbols-outlined text-[14px]">edit</span></button>
                                      <button onClick={() => deleteTranscript(transcript.id)} className="p-1 rounded text-on-surface-variant/80 hover:text-error transition-colors"><span className="material-symbols-outlined text-[14px]">close</span></button>
                                    </div>
                                  </div>
                                )}
                              </div>
                            );
                          })}
                        </div>
                      ));
                    })()}
                  </div>
                )}
              </section>
            )}

            {/* ── PROFILES ── lista con radio visual inline */}
            {activeTab === "profiles" && (
              <section>
                <div className="flex items-baseline gap-2 mb-6">
                  <h3 className="text-xl font-black text-on-surface font-headline">{t.transformation_profiles}</h3>
                </div>
                <div>
                  {profiles.map(profile => {
                    const isActive = settings.active_profile_id === profile.id.toString();
                    return (
                      <div key={profile.id}>
                        <div onClick={() => updateSetting("active_profile_id", profile.id.toString())} className="group flex items-center gap-3 py-2.5 border-b border-on-surface/[0.08] last:border-0 cursor-pointer hover:bg-surface-container transition-colors rounded-sm -mx-2 px-2">
                          <div className={`w-3.5 h-3.5 rounded-full border flex-shrink-0 flex items-center justify-center transition-all ${isActive ? 'border-primary bg-primary' : 'border-on-surface-variant/20 bg-transparent'}`}>
                            {isActive && <div className="w-1.5 h-1.5 rounded-full bg-background" />}
                          </div>
                          <span className={`text-xs font-black uppercase tracking-widest flex-shrink-0 w-28 ${isActive ? 'text-on-surface font-bold' : 'text-on-surface-variant'}`}>{profile.name}</span>
                          <span className="flex-1 text-[11px] italic text-on-surface-variant truncate min-w-0">{profile.system_prompt || t.exact_transcription}</span>
                          <button onClick={(e) => { e.stopPropagation(); if (editingProfileId === profile.id) { setEditingProfileId(null); } else { setEditingProfileId(profile.id); setEditName(profile.name); setEditPrompt(profile.system_prompt); setEditIcon(profile.icon || 'psychology'); setIsCreatingProfile(false); } }} className="opacity-0 group-hover:opacity-100 p-1 rounded text-on-surface-variant hover:text-on-surface transition-all flex-shrink-0">
                            <span className="material-symbols-outlined text-[14px]">edit</span>
                          </button>
                        </div>
                        {editingProfileId === profile.id && (
                          <div className="ml-6 mt-1 mb-3 p-5 rounded-xl bg-surface-container-high/60 backdrop-blur-xl space-y-4 animate-in slide-in-from-top-2 duration-200">
                            <div className="grid grid-cols-2 gap-3">
                              <div className="space-y-1.5">
                                <label className="text-[9px] font-black text-on-surface-variant uppercase tracking-widest">{t.name_label}</label>
                                <input type="text" value={editName} onChange={(e) => setEditName(e.target.value)} className="w-full bg-background/40 rounded-lg px-3 py-2 text-xs text-on-surface focus:outline-none focus:ring-1 focus:ring-primary/30 transition-all font-bold" />
                              </div>
                              <div className="space-y-1.5">
                                <label className="text-[9px] font-black text-on-surface-variant uppercase tracking-widest">{t.icon_label}</label>
                                <div className="flex flex-wrap gap-1.5 p-2 bg-primary/5 rounded-lg max-h-[80px] overflow-y-auto custom-scrollbar">
                                  {AVAILABLE_ICONS.map(icon => (
                                    <button key={icon} onClick={() => setEditIcon(icon)} className={`p-1.5 rounded-lg transition-all ${editIcon === icon ? 'bg-primary text-on-primary' : 'text-on-surface-variant hover:text-on-surface hover:bg-surface-container'}`}>
                                      <span className="material-symbols-outlined text-[16px]">{icon}</span>
                                    </button>
                                  ))}
                                </div>
                              </div>
                            </div>
                            <div className="space-y-1.5">
                              <label className="text-[9px] font-black text-on-surface-variant uppercase tracking-widest">{t.prompt_label}</label>
                              <textarea value={editPrompt} onChange={(e) => setEditPrompt(e.target.value)} rows={3} className="w-full bg-background/40 rounded-lg px-3 py-2 text-xs text-on-surface focus:outline-none focus:ring-1 focus:ring-primary/30 transition-all resize-none leading-relaxed font-medium italic" placeholder={t.prompt_placeholder} />
                            </div>
                            <div className="space-y-1.5">
                              <label className="text-[9px] font-black text-on-surface-variant uppercase tracking-widest">{t.formatting_mode_label}</label>
                              <div className="flex gap-2">
                                {(['plain', 'markdown'] as const).map(mode => {
                                  const isModeActive = (profile.formatting_mode || 'plain') === mode;
                                  return (
                                    <button key={mode} onClick={() => updateProfileFormattingMode(profile.id, mode)} className={`flex-1 py-1.5 rounded-lg text-[10px] font-black uppercase tracking-wider transition-all ${isModeActive ? 'bg-primary text-background' : 'bg-background/40 text-on-surface-variant hover:bg-surface-container'}`}>
                                      <div>{mode === 'plain' ? t.formatting_mode_plain : t.formatting_mode_markdown}</div>
                                      <div className={`text-[8px] normal-case tracking-normal mt-0.5 font-normal ${isModeActive ? 'text-on-primary/70' : 'text-on-surface-variant'}`}>{mode === 'plain' ? t.formatting_mode_plain_desc : t.formatting_mode_markdown_desc}</div>
                                    </button>
                                  );
                                })}
                              </div>
                            </div>
                            <div className="flex gap-2 pt-1">
                              <button onClick={() => { updateProfile(profile.id, editName, editPrompt, editIcon); setEditingProfileId(null); }} className="flex-1 bg-on-surface text-background py-2 rounded-lg text-[10px] font-black uppercase tracking-wider hover:bg-on-surface/90 transition-all">{t.save_profile}</button>
                              {!profile.is_default && <button onClick={() => setConfirmModal({ type: 'delete-profile', id: profile.id, name: profile.name })} className="px-4 bg-error/10 text-error py-2 rounded-lg text-[10px] font-black uppercase tracking-wider hover:bg-error/20 transition-all">{t.borrar}</button>}
                              <button onClick={() => setEditingProfileId(null)} className="px-4 bg-surface-container text-on-surface-variant py-2 rounded-lg text-[10px] font-black uppercase tracking-wider hover:bg-surface-container-high transition-all">{t.cancel}</button>
                            </div>
                          </div>
                        )}
                      </div>
                    );
                  })}
                  {!isCreatingProfile ? (
                    <button onClick={() => { setIsCreatingProfile(true); setEditingProfileId(null); setNewName(""); setNewPrompt(""); setNewIcon("psychology"); }} className="mt-3 flex items-center gap-2 text-[11px] font-black uppercase tracking-wider text-on-surface-variant/80 hover:text-primary transition-colors py-2">
                      <span className="material-symbols-outlined text-[16px]">add</span>
                      {t.create_new_profile}
                    </button>
                  ) : (
                    <div className="mt-3 p-5 rounded-xl bg-primary/5 space-y-4 animate-in zoom-in-95 duration-200">
                      <div className="grid grid-cols-2 gap-3">
                        <div className="space-y-1.5">
                          <label className="text-[9px] font-black text-on-surface-variant uppercase tracking-widest">{t.name_label}</label>
                          <input type="text" placeholder={t.writer_example} value={newName} onChange={(e) => setNewName(e.target.value)} className="w-full bg-background/40 rounded-lg px-3 py-2 text-xs text-on-surface focus:outline-none focus:ring-1 focus:ring-primary/30 transition-all font-bold" />
                        </div>
                        <div className="space-y-1.5">
                          <label className="text-[9px] font-black text-on-surface-variant uppercase tracking-widest">{t.icon_label}</label>
                          <div className="flex flex-wrap gap-1.5 p-2 bg-primary/5 rounded-lg max-h-[80px] overflow-y-auto custom-scrollbar">
                            {AVAILABLE_ICONS.map(icon => (
                              <button key={icon} onClick={() => setNewIcon(icon)} className={`p-1.5 rounded-lg transition-all ${newIcon === icon ? 'bg-primary text-on-primary' : 'text-on-surface-variant hover:text-on-surface hover:bg-surface-container'}`}>
                                <span className="material-symbols-outlined text-[16px]">{icon}</span>
                              </button>
                            ))}
                          </div>
                        </div>
                      </div>
                      <div className="space-y-1.5">
                        <label className="text-[9px] font-black text-on-surface-variant uppercase tracking-widest">{t.instructions_custom}</label>
                        <textarea value={newPrompt} onChange={(e) => setNewPrompt(e.target.value)} rows={3} className="w-full bg-background/40 rounded-lg px-3 py-2 text-xs text-on-surface focus:outline-none focus:ring-1 focus:ring-primary/30 transition-all resize-none leading-relaxed font-medium italic" placeholder={t.expert_example} />
                      </div>
                      <div className="flex gap-2">
                        <button onClick={() => { if (newName && newPrompt) { createProfile(newName, newPrompt, newIcon); setIsCreatingProfile(false); } }} className="flex-1 bg-on-surface text-background py-2 rounded-lg text-[10px] font-black uppercase tracking-wider hover:bg-on-surface/90 transition-all">{t.create_profile}</button>
                        <button onClick={() => setIsCreatingProfile(false)} className="px-4 bg-surface-container text-on-surface-variant py-2 rounded-lg text-[10px] font-black uppercase tracking-wider hover:bg-surface-container-high transition-all">{t.discard}</button>
                      </div>
                    </div>
                  )}
                </div>
              </section>
            )}

            {/* ── DICTIONARY ── tabla ultra-compacta */}
            {activeTab === "dictionary" && (
              <section className="flex flex-col">
                <div className="flex items-baseline gap-2 mb-6">
                  <h3 className="text-xl font-black text-on-surface font-headline">{t.personal_dictionary}</h3>
                  {dictionaryEntries.length > 0 && <span className="text-[10px] text-on-surface-variant font-mono">({dictionaryEntries.length})</span>}
                </div>
                <div className="flex items-center gap-4 pb-2 border-b border-on-surface/[0.10]">
                  <span className="flex-1 text-[9px] font-black uppercase tracking-[0.3em] text-on-surface-variant/80">{t.word ?? "Word"}</span>
                  <span className="flex-1 text-[9px] font-black uppercase tracking-[0.3em] text-on-surface-variant/80">{t.replacement ?? "Replacement"}</span>
                  <span className="w-10 text-right text-[9px] font-black uppercase tracking-[0.3em] text-on-surface-variant/80">{t.usage ?? "Uses"}</span>
                  <span className="w-4" />
                </div>
                {dictionaryEntries.length === 0 ? (
                  <div className="py-10 text-center"><p className="text-[10px] text-on-surface-variant/80 italic font-black uppercase tracking-[0.2em]">{t.dictionary_empty}</p></div>
                ) : (
                  <div>
                    {dictionaryEntries.map(entry => (
                      <div key={entry.word} className="group flex items-center gap-4 py-2 border-b border-on-surface/[0.08] last:border-0 min-h-[36px]">
                        <span className="flex-1 text-sm font-bold text-on-surface">{entry.word}</span>
                        <input type="text" placeholder="—" defaultValue={entry.replacement_word ?? ""} onBlur={(e) => { const val = e.target.value.trim() || null; updateReplacement(entry.word, val); }} onKeyDown={(e) => { if (e.key === 'Enter') (e.target as HTMLInputElement).blur(); }} className="flex-1 bg-transparent text-xs text-on-surface/75 placeholder:text-on-surface-variant/80 focus:outline-none focus:bg-surface-container rounded px-1 py-0.5 transition-colors" />
                        <span className="w-10 text-right">
                          {entry.usage_count > 0 ? <span className="text-[10px] font-black text-primary/90">{entry.usage_count}</span> : <span className="text-on-surface-variant/60 text-[10px]">—</span>}
                        </span>
                        <button onClick={() => removeWord(entry.word)} className="w-4 opacity-0 group-hover:opacity-100 text-on-surface-variant/80 hover:text-error transition-all">
                          <span className="material-symbols-outlined text-[14px]">close</span>
                        </button>
                      </div>
                    ))}
                  </div>
                )}
                <div className="sticky bottom-0 pt-3 pb-1 bg-background/80 backdrop-blur-sm flex gap-2 mt-4">
                  <input type="text" placeholder={t.dictionary_placeholder} value={newWord} onChange={(e) => setNewWord(e.target.value)} onKeyDown={(e) => { if (e.key === 'Enter' && newWord.trim()) { addWord(newWord.trim()); setNewWord(""); } }} className="flex-1 bg-surface-container rounded-lg px-3 py-2 text-sm text-on-surface focus:outline-none focus:ring-1 focus:ring-primary/30 placeholder:text-on-surface-variant/80" />
                  <button onClick={() => { if (newWord.trim()) { addWord(newWord.trim()); setNewWord(""); } }} disabled={!newWord.trim()} className="px-5 bg-primary text-background rounded-lg text-[11px] font-black uppercase tracking-wider disabled:opacity-30 disabled:cursor-not-allowed hover:bg-primary/90 transition-all">{t.add}</button>
                </div>
              </section>
            )}

            {/* ── MODELS ── lista simple */}
            {activeTab === "models" && (
              <section>
                <div className="flex items-baseline gap-2 mb-6">
                  <h3 className="text-xl font-black text-on-surface font-headline">{t.models}</h3>
                </div>
                {modelsInfo && (
                  <div>
                    <div className="text-[9px] font-bold uppercase tracking-[0.3em] text-on-surface-variant pb-2 border-b border-on-surface/[0.10]">{t.path}</div>
                    <div className="flex items-center justify-between py-2.5 border-b border-on-surface/[0.08]">
                      <span className="font-mono text-[11px] text-on-surface-variant truncate flex-1 mr-3">{modelsInfo.base_path}</span>
                      <div className="flex gap-1 flex-shrink-0">
                        <CopyButton text={modelsInfo.base_path} copyLabel={t.path} />
                        <button onClick={() => invoke("open_models_folder")} className="p-1 rounded text-on-surface-variant/80 hover:text-primary transition-colors"><span className="material-symbols-outlined text-[14px]">folder_open</span></button>
                      </div>
                    </div>
                    <div className="text-[9px] font-bold uppercase tracking-[0.3em] text-on-surface-variant pt-5 pb-2 mt-1 border-t border-on-surface/[0.10]">{t.ai_models}</div>
                    {modelsInfo.models.map((model: any) => (
                      <div key={model.filename} className="flex items-center gap-4 py-2.5 border-b border-on-surface/[0.08] last:border-0 min-h-[40px]">
                        <span className="flex-1 text-sm font-bold text-on-surface">{model.display_name}</span>
                        <span className="font-mono text-[10px] text-on-surface-variant/80">{model.filename}</span>
                        <span className="text-[11px] text-on-surface-variant w-16 text-right">{model.size_mb} {t.size_mb}</span>
                        <span className={`text-[9px] font-black uppercase tracking-wider px-2 py-0.5 rounded-full flex-shrink-0 ${model.downloaded ? 'bg-primary/10 text-primary/90' : 'bg-error/10 text-error/90'}`}>{model.downloaded ? t.downloaded : t.missing}</span>
                      </div>
                    ))}
                    {isDownloadingModels && downloadProgress && (
                      <div className="mt-4 space-y-1.5">
                        <div className="flex justify-between text-[9px] font-black uppercase tracking-wider text-primary/90">
                          <span>{downloadProgress.model}</span><span>{downloadProgress.progress.toFixed(0)}%</span>
                        </div>
                        <div className="h-[3px] bg-surface-container-highest rounded-full overflow-hidden">
                          <div className="h-full bg-primary rounded-full transition-all duration-300 shadow-[0_0_6px_rgba(157,122,255,0.5)]" style={{ width: `${downloadProgress.progress}%` }} />
                        </div>
                      </div>
                    )}
                    <div className="flex justify-end pt-5">
                      <button onClick={() => setConfirmModal({ type: 'redownload' })} disabled={isDownloadingModels} className="text-[10px] font-black uppercase tracking-wider text-on-surface-variant hover:text-on-surface disabled:opacity-20 disabled:cursor-not-allowed transition-colors py-1.5 px-3 rounded-lg hover:bg-surface-container-high/60">
                        {isDownloadingModels ? t.loading : t.redownload}
                      </button>
                    </div>
                  </div>
                )}
              </section>
            )}

            {/* ── GENERAL ── rows inline, sin tarjetas */}
            {activeTab === "general" && (
              <section>
                <div className="flex items-baseline gap-2 mb-6">
                  <h3 className="text-xl font-black text-on-surface font-headline">{t.system_settings}</h3>
                </div>
                <div className="text-[9px] font-bold uppercase tracking-[0.3em] text-on-surface-variant pb-2 border-b border-on-surface/[0.10]">{t.audio_source}</div>
                <div className="flex items-center justify-between py-2.5 min-h-[40px] border-b border-on-surface/[0.08]">
                  <span className="text-sm text-on-surface">{t.audio_source}</span>
                  <div className="relative flex-shrink-0">
                    <select value={settings.mic_id} onChange={(e) => updateSetting("mic_id", e.target.value)} className="bg-surface-container-high/60 text-on-surface text-xs font-medium rounded-lg pl-3 pr-7 py-1.5 appearance-none cursor-pointer focus:outline-none focus:ring-1 focus:ring-primary/30 hover:bg-surface-container-highest/70 transition-colors">
                      <option value="auto">{t.auto_detect}</option>
                      {micDevices.map(dev => <option key={dev.id} value={dev.id}>{dev.name}</option>)}
                    </select>
                    <span className="absolute right-2 top-1/2 -translate-y-1/2 pointer-events-none text-on-surface-variant material-symbols-outlined text-[14px]">expand_more</span>
                  </div>
                </div>
                <div className="flex items-center justify-between pt-5 pb-2 mt-1 border-t border-on-surface/[0.10]">
                  <span className="text-[9px] font-bold uppercase tracking-[0.3em] text-on-surface-variant">{t.global_shortcut}</span>
                  <button onClick={() => { updateSetting("shortcut_push_to_talk", "Alt+Space"); updateSetting("shortcut_hands_free", "F5"); updateSetting("shortcut_paste", "CommandOrControl+Shift+V"); updateSetting("shortcut_cancel", "Escape"); }} className="text-[9px] font-black uppercase tracking-wider text-on-surface-variant/80 hover:text-on-surface transition-colors">{t.reset_defaults}</button>
                </div>
                {[
                  { key: "shortcut_push_to_talk", label: t.shortcut_push_to_talk },
                  { key: "shortcut_hands_free", label: t.shortcut_hands_free },
                  { key: "shortcut_paste", label: t.shortcut_paste },
                  { key: "shortcut_cancel", label: t.shortcut_cancel }
                ].map((sc) => {
                  const isCapturing = capturingShortcutFor === sc.key;
                  const currentVal = settings[sc.key as keyof AppSettings] as string || "";
                  const displayVal = isCapturing ? t.listening : currentVal.replace("CommandOrControl", "⌘").replace("Alt", "⌥").replace("Shift", "⇧").replace("Escape", "Esc");
                  return (
                    <div key={sc.key} className="flex items-center justify-between py-2.5 min-h-[40px] border-b border-on-surface/[0.08]">
                      <span className="text-sm text-on-surface">{sc.label}</span>
                      <button onClick={() => setCapturingShortcutFor(sc.key as keyof AppSettings)} className={`font-mono text-sm px-2.5 py-1 rounded-md transition-all ${isCapturing ? 'bg-primary/10 text-primary ring-1 ring-primary/30 animate-pulse' : 'bg-surface-container text-on-surface hover:bg-surface-container-highest/70'}`}>{displayVal}</button>
                    </div>
                  );
                })}
                <div className="text-[9px] font-bold uppercase tracking-[0.3em] text-on-surface-variant pt-5 pb-2 mt-1 border-t border-on-surface/[0.10]">Behavior</div>
                <div className="flex items-center justify-between py-2.5 min-h-[40px] border-b border-on-surface/[0.08]">
                  <div>
                    <span className="text-sm text-on-surface">{t.auto_detect_profile}</span>
                    <p className="text-[10px] text-on-surface-variant/80 mt-0.5">{t.auto_detect_profile_hint}</p>
                  </div>
                  <button onClick={() => { const next = settings.auto_detect_profile !== "true" ? "true" : "false"; updateSetting("auto_detect_profile" as keyof AppSettings, next); }} className={`relative w-10 h-[22px] rounded-full transition-colors flex-shrink-0 ml-4 ${settings.auto_detect_profile !== "false" ? "bg-primary" : "bg-surface-container-highest"}`}>
                    <span className={`absolute top-[3px] w-4 h-4 rounded-full bg-white shadow-sm transition-transform ${settings.auto_detect_profile !== "false" ? "translate-x-[22px]" : "translate-x-[3px]"}`} />
                  </button>
                </div>
                <div className="flex items-center justify-between py-2.5 min-h-[40px]">
                  <span className="text-sm text-on-surface">{t.transcription_input_lang}</span>
                  <div className="flex gap-1.5">
                    {['es', 'en'].map(lang => (
                      <button key={lang} onClick={() => updateSetting("language", lang)} className={`px-3 py-1 rounded-full text-[11px] font-black uppercase tracking-wider transition-all ${settings.language === lang ? 'bg-primary text-background' : 'text-on-surface-variant hover:text-on-surface hover:bg-surface-container'}`}>
                        {lang === 'es' ? t.spanish : t.english}
                      </button>
                    ))}
                  </div>
                </div>
              </section>
            )}

          </div>
        </main>
      </div>

      {/* ── FOOTER ── 32px transparente */}
      <footer className="h-8 flex items-center px-8 border-t border-on-surface/[0.08] flex-shrink-0">
        <span className="text-[9px] font-mono text-on-surface-variant/70 tracking-widest">{t.footer_engine.replace("{version}", appVersion)}</span>
      </footer>

      {/* ── CONFIRM MODAL ── glassmorphism */}
      {confirmModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm animate-in fade-in duration-150">
          <div className="bg-surface-container-low/90 backdrop-blur-2xl rounded-2xl ring-1 ring-white/[0.06] p-6 max-w-xs w-full mx-6 space-y-4 animate-in zoom-in-95 duration-200 shadow-2xl">
            <div className="space-y-1.5">
              <h3 className="text-sm font-black text-on-surface uppercase tracking-widest">{confirmModal.type === 'redownload' ? t.redownload : t.delete}</h3>
              <p className="text-xs text-on-surface/75 leading-relaxed">
                {confirmModal.type === 'clear' ? t.confirm_clear : confirmModal.type === 'redownload' ? t.redownload + "?" : confirmModal.type === 'delete-profile' ? t.confirm_delete_profile.replace("{name}", confirmModal.name) : t.confirm_delete_transcript}
              </p>
            </div>
            <div className="flex gap-2 justify-end pt-1">
              <button onClick={() => setConfirmModal(null)} className="px-4 py-2 rounded-lg bg-surface-container-high/60 text-on-surface-variant text-[10px] font-black uppercase tracking-wider hover:bg-surface-container-high/60 transition-colors">{t.cancel}</button>
              <button onClick={() => { if (confirmModal.type === 'redownload') executeRedownload(); else if (confirmModal.type === 'clear') executeClearHistory(); else if (confirmModal.type === 'delete-profile') { deleteProfile(confirmModal.id); setConfirmModal(null); setEditingProfileId(null); } else executeDelete(confirmModal.id); }} className="px-4 py-2 rounded-lg bg-error/80 text-white text-[10px] font-black uppercase tracking-wider hover:bg-error/90 transition-colors">
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
      className={`p-2 rounded-lg transition-all ${copied ? 'bg-primary/10 text-primary' : 'bg-surface-container-low/70 text-on-surface-variant/80 hover:text-on-surface hover:bg-surface-container-high/60'}`}
      title={copyLabel}
    >
      <span className="material-symbols-outlined text-[18px]">
        {copied ? 'done' : 'content_copy'}
      </span>
    </button>
  );
}

export default SettingsPanel;
