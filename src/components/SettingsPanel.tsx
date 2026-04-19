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
                    <button onClick={clearHistory} className="text-[9px] font-black uppercase tracking-widest px-3 py-1.5 rounded-lg bg-error/10 text-error border border-error/20 hover:bg-error/20 transition-all">
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
                                      <button onClick={() => { setEditingTranscriptId(null); setLearnedWords([]); }} className="px-4 bg-surface-container text-on-surface-variant py-2.5 rounded-xl text-[10px] font-black uppercase tracking-wider border border-on-surface/[0.10] hover:bg-surface-container-high transition-all">{t.cancel}</button>
                                    </div>
                                  </div>
                                ) : (
                                  <div className="p-4 mb-3 rounded-xl bg-surface-container-low border border-on-surface/[0.06] hover:border-primary/20 hover:bg-surface-container transition-all">
                                    <div className="flex items-center justify-between mb-2">
                                      <span className="font-mono text-[10px] text-on-surface-variant font-medium">{time}</span>
                                      <div className="flex items-center gap-1">
                                        <CopyButton text={transcript.content} copyLabel={t.copy_text} />
                                        <button onClick={() => startEditTranscript(transcript.id, transcript.content)} className="p-1.5 rounded-lg bg-surface-container text-on-surface-variant hover:bg-primary/10 hover:text-primary transition-all" title={t.edit ?? "Edit"}><span className="material-symbols-outlined text-[14px]">edit</span></button>
                                        <button onClick={() => deleteTranscript(transcript.id)} className="p-1.5 rounded-lg bg-surface-container text-on-surface-variant hover:bg-error/10 hover:text-error transition-all" title={t.delete}><span className="material-symbols-outlined text-[14px]">delete</span></button>
                                      </div>
                                    </div>
                                    <p className={`text-sm text-on-surface leading-relaxed cursor-pointer ${isExpanded ? 'whitespace-pre-wrap' : ''}`} onClick={() => setExpandedTranscriptId(isExpanded ? null : transcript.id)}>{transcript.content}</p>
                                    {!isExpanded && transcript.content.length > 120 && (
                                      <button onClick={() => setExpandedTranscriptId(transcript.id)} className="mt-1 text-[10px] text-primary font-semibold hover:text-primary/80 transition-colors">Ver más...</button>
                                    )}
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

            {/* ── PROFILES ── tarjetas con icono, nombre y descripción */}
            {activeTab === "profiles" && (
              <section>
                <div className="flex items-baseline gap-2 mb-6">
                  <h3 className="text-xl font-black text-on-surface font-headline">{t.transformation_profiles}</h3>
                </div>
                <div className="grid gap-3">
                  {profiles.map(profile => {
                    const isActive = settings.active_profile_id === profile.id.toString();
                    const isEditing = editingProfileId === profile.id;
                    const promptPreview = (() => {
                      const descriptions: Record<string, string> = {
                        'Elegant': 'Formal and polished writing with perfect grammar',
                        'Informal': 'Casual and direct, like a chat message',
                        'Code': 'Transforms voice into structured AI prompts',
                        'Raw': 'Exact transcription without any changes',
                      };
                      return descriptions[profile.name] || (profile.system_prompt
                        ? (profile.system_prompt.length > 60 ? profile.system_prompt.slice(0, 60) + '...' : profile.system_prompt)
                        : t.exact_transcription);
                    })();
                    return (
                      <div key={profile.id}>
                        {/* Profile card */}
                        <div
                          onClick={() => updateSetting("active_profile_id", profile.id.toString())}
                          className={`relative p-4 rounded-2xl border cursor-pointer transition-all ${
                            isActive
                              ? 'bg-primary/[0.06] border-primary/30 shadow-sm'
                              : 'bg-surface-container-low border-on-surface/[0.08] hover:bg-surface-container hover:border-on-surface/[0.15]'
                          }`}
                        >
                          <div className="flex items-start gap-3">
                            {/* Icon */}
                            <div className={`w-10 h-10 rounded-xl flex items-center justify-center flex-shrink-0 ${
                              isActive ? 'bg-primary/15 text-primary' : 'bg-surface-container text-on-surface-variant'
                            }`}>
                              <span className="material-symbols-outlined text-[22px] material-symbols-fill">{profile.icon || 'psychology'}</span>
                            </div>
                            {/* Content */}
                            <div className="flex-1 min-w-0">
                              <div className="flex items-center gap-2">
                                <span className={`text-sm font-bold ${isActive ? 'text-on-surface' : 'text-on-surface-variant'}`}>{profile.name}</span>
                                {isActive && (
                                  <span className="px-2 py-0.5 rounded-full bg-primary/15 text-[9px] font-bold text-primary uppercase tracking-wider">{t.active}</span>
                                )}
                              </div>
                              <p className="text-[11px] text-on-surface-variant mt-1 leading-relaxed">{promptPreview}</p>
                            </div>
                            {/* Edit button */}
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                if (isEditing) { setEditingProfileId(null); }
                                else { setEditingProfileId(profile.id); setEditName(profile.name); setEditPrompt(profile.system_prompt); setEditIcon(profile.icon || 'psychology'); setIsCreatingProfile(false); }
                              }}
                              className={`p-2 rounded-xl flex-shrink-0 transition-all ${
                                isEditing ? 'bg-primary/10 text-primary' : 'text-on-surface-variant hover:text-on-surface hover:bg-surface-container'
                              }`}
                            >
                              <span className="material-symbols-outlined text-[16px]">{isEditing ? 'close' : 'tune'}</span>
                            </button>
                          </div>
                        </div>

                        {/* Edit drawer — expands below the card */}
                        {isEditing && (
                          <div className="mt-1 p-5 rounded-2xl bg-surface-container border border-on-surface/[0.08] space-y-4 animate-in slide-in-from-top-2 duration-200">
                            <div className="grid grid-cols-2 gap-3">
                              <div className="space-y-1.5">
                                <label className="text-[9px] font-black text-on-surface-variant uppercase tracking-widest">{t.name_label}</label>
                                <input type="text" value={editName} onChange={(e) => setEditName(e.target.value)} className="w-full bg-background rounded-xl px-3 py-2 text-xs text-on-surface border border-on-surface/[0.10] focus:outline-none focus:ring-2 focus:ring-primary/30 focus:border-primary/40 transition-all font-bold" />
                              </div>
                              <div className="space-y-1.5">
                                <label className="text-[9px] font-black text-on-surface-variant uppercase tracking-widest">{t.icon_label}</label>
                                <div className="flex flex-wrap gap-1.5 p-2 bg-surface-container-low rounded-xl max-h-[80px] overflow-y-auto custom-scrollbar border border-on-surface/[0.06]">
                                  {AVAILABLE_ICONS.map(icon => (
                                    <button key={icon} onClick={() => setEditIcon(icon)} className={`p-1.5 rounded-lg transition-all ${editIcon === icon ? 'bg-primary text-on-primary shadow-sm' : 'text-on-surface-variant hover:text-on-surface hover:bg-surface-container-high'}`}>
                                      <span className="material-symbols-outlined text-[16px]">{icon}</span>
                                    </button>
                                  ))}
                                </div>
                              </div>
                            </div>
                            <div className="space-y-1.5">
                              <label className="text-[9px] font-black text-on-surface-variant uppercase tracking-widest">{t.prompt_label}</label>
                              <textarea value={editPrompt} onChange={(e) => setEditPrompt(e.target.value)} rows={3} className="w-full bg-background rounded-xl px-3 py-2 text-xs text-on-surface border border-on-surface/[0.10] focus:outline-none focus:ring-2 focus:ring-primary/30 focus:border-primary/40 transition-all resize-none leading-relaxed" placeholder={t.prompt_placeholder} />
                            </div>
                            <div className="space-y-1.5">
                              <label className="text-[9px] font-black text-on-surface-variant uppercase tracking-widest">{t.formatting_mode_label}</label>
                              <div className="flex gap-2">
                                {(['plain', 'markdown'] as const).map(mode => {
                                  const isModeActive = (profile.formatting_mode || 'plain') === mode;
                                  return (
                                    <button key={mode} onClick={() => updateProfileFormattingMode(profile.id, mode)} className={`flex-1 py-2 rounded-xl text-[10px] font-black uppercase tracking-wider transition-all ${isModeActive ? 'bg-primary text-background shadow-sm' : 'bg-surface-container-low text-on-surface-variant border border-on-surface/[0.10] hover:bg-surface-container-high'}`}>
                                      <div>{mode === 'plain' ? t.formatting_mode_plain : t.formatting_mode_markdown}</div>
                                      <div className={`text-[8px] normal-case tracking-normal mt-0.5 font-normal ${isModeActive ? 'text-on-primary/70' : 'text-on-surface-variant'}`}>{mode === 'plain' ? t.formatting_mode_plain_desc : t.formatting_mode_markdown_desc}</div>
                                    </button>
                                  );
                                })}
                              </div>
                            </div>
                            <div className="flex gap-2 pt-1">
                              <button onClick={() => { updateProfile(profile.id, editName, editPrompt, editIcon); setEditingProfileId(null); }} className="flex-1 bg-primary text-background py-2.5 rounded-xl text-[10px] font-black uppercase tracking-wider hover:bg-primary/90 transition-all shadow-sm">{t.save_profile}</button>
                              {!profile.is_default && <button onClick={() => setConfirmModal({ type: 'delete-profile', id: profile.id, name: profile.name })} className="px-4 bg-error/10 text-error py-2.5 rounded-xl text-[10px] font-black uppercase tracking-wider border border-error/20 hover:bg-error/20 transition-all">{t.borrar}</button>}
                              <button onClick={() => setEditingProfileId(null)} className="px-4 bg-surface-container-low text-on-surface-variant py-2.5 rounded-xl text-[10px] font-black uppercase tracking-wider border border-on-surface/[0.10] hover:bg-surface-container-high transition-all">{t.cancel}</button>
                            </div>
                          </div>
                        )}
                      </div>
                    );
                  })}

                  {/* Create new profile */}
                  {!isCreatingProfile ? (
                    <button onClick={() => { setIsCreatingProfile(true); setEditingProfileId(null); setNewName(""); setNewPrompt(""); setNewIcon("psychology"); }} className="p-4 rounded-2xl border-2 border-dashed border-on-surface/[0.12] text-on-surface-variant hover:border-primary hover:text-primary hover:bg-primary/5 transition-all flex items-center justify-center gap-2">
                      <span className="material-symbols-outlined text-[18px]">add</span>
                      <span className="text-[11px] font-black uppercase tracking-wider">{t.create_new_profile}</span>
                    </button>
                  ) : (
                    <div className="p-5 rounded-2xl bg-primary/5 border border-primary/15 space-y-4 animate-in zoom-in-95 duration-200">
                      <div className="grid grid-cols-2 gap-3">
                        <div className="space-y-1.5">
                          <label className="text-[9px] font-black text-on-surface-variant uppercase tracking-widest">{t.name_label}</label>
                          <input type="text" placeholder={t.writer_example} value={newName} onChange={(e) => setNewName(e.target.value)} className="w-full bg-background rounded-xl px-3 py-2 text-xs text-on-surface border border-on-surface/[0.10] focus:outline-none focus:ring-2 focus:ring-primary/30 focus:border-primary/40 transition-all font-bold" />
                        </div>
                        <div className="space-y-1.5">
                          <label className="text-[9px] font-black text-on-surface-variant uppercase tracking-widest">{t.icon_label}</label>
                          <div className="flex flex-wrap gap-1.5 p-2 bg-surface-container-low rounded-xl max-h-[80px] overflow-y-auto custom-scrollbar border border-on-surface/[0.06]">
                            {AVAILABLE_ICONS.map(icon => (
                              <button key={icon} onClick={() => setNewIcon(icon)} className={`p-1.5 rounded-lg transition-all ${newIcon === icon ? 'bg-primary text-on-primary shadow-sm' : 'text-on-surface-variant hover:text-on-surface hover:bg-surface-container-high'}`}>
                                <span className="material-symbols-outlined text-[16px]">{icon}</span>
                              </button>
                            ))}
                          </div>
                        </div>
                      </div>
                      <div className="space-y-1.5">
                        <label className="text-[9px] font-black text-on-surface-variant uppercase tracking-widest">{t.instructions_custom}</label>
                        <textarea value={newPrompt} onChange={(e) => setNewPrompt(e.target.value)} rows={3} className="w-full bg-background rounded-xl px-3 py-2 text-xs text-on-surface border border-on-surface/[0.10] focus:outline-none focus:ring-2 focus:ring-primary/30 focus:border-primary/40 transition-all resize-none leading-relaxed" placeholder={t.expert_example} />
                      </div>
                      <div className="flex gap-2">
                        <button onClick={() => { if (newName && newPrompt) { createProfile(newName, newPrompt, newIcon); setIsCreatingProfile(false); } }} className="flex-1 bg-primary text-background py-2.5 rounded-xl text-[10px] font-black uppercase tracking-wider hover:bg-primary/90 transition-all shadow-sm">{t.create_profile}</button>
                        <button onClick={() => setIsCreatingProfile(false)} className="px-4 bg-surface-container-low text-on-surface-variant py-2.5 rounded-xl text-[10px] font-black uppercase tracking-wider border border-on-surface/[0.10] hover:bg-surface-container-high transition-all">{t.discard}</button>
                      </div>
                    </div>
                  )}
                </div>
              </section>
            )}

            {/* ── DICTIONARY ── add at top, paginated grid */}
            {activeTab === "dictionary" && (
              <DictionarySection
                dictionaryEntries={dictionaryEntries}
                newWord={newWord}
                setNewWord={setNewWord}
                addWord={addWord}
                removeWord={removeWord}
                updateReplacement={updateReplacement}
                t={t}
              />
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
                      <button onClick={() => setConfirmModal({ type: 'redownload' })} disabled={isDownloadingModels} className="text-[10px] font-black uppercase tracking-wider px-4 py-2 rounded-xl bg-surface-container text-on-surface-variant border border-on-surface/[0.10] hover:bg-surface-container-high hover:text-on-surface disabled:opacity-40 disabled:cursor-not-allowed transition-all">
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
                  <button onClick={() => { updateSetting("shortcut_push_to_talk", "Alt+Space"); updateSetting("shortcut_hands_free", "F5"); updateSetting("shortcut_paste", "CommandOrControl+Shift+V"); updateSetting("shortcut_cancel", "Escape"); }} className="text-[9px] font-black uppercase tracking-wider px-3 py-1.5 rounded-lg bg-surface-container text-on-surface-variant border border-on-surface/[0.10] hover:bg-surface-container-high hover:text-on-surface transition-all">{t.reset_defaults}</button>
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
                      <button onClick={() => setCapturingShortcutFor(sc.key as keyof AppSettings)} className={`font-mono text-sm px-2.5 py-1 rounded-md transition-all ${isCapturing ? 'bg-primary/10 text-primary ring-2 ring-primary/40 animate-pulse' : 'bg-surface-container-high text-on-surface font-bold border border-on-surface/[0.12] hover:bg-surface-container-highest hover:border-primary/30 shadow-sm'}`}>{displayVal}</button>
                    </div>
                  );
                })}
                <div className="text-[9px] font-bold uppercase tracking-[0.3em] text-on-surface-variant pt-5 pb-2 mt-1 border-t border-on-surface/[0.10]">Behavior</div>
                <div className="flex items-center justify-between py-3 min-h-[48px] border-b border-on-surface/[0.08]">
                  <div className="flex-1 mr-4">
                    <span className="text-sm font-medium text-on-surface">{t.auto_detect_profile}</span>
                    <p className="text-[11px] text-on-surface-variant mt-0.5 leading-relaxed">{t.auto_detect_profile_hint}</p>
                  </div>
                  <button
                    onClick={() => { const next = settings.auto_detect_profile !== "true" ? "true" : "false"; updateSetting("auto_detect_profile" as keyof AppSettings, next); }}
                    className={`relative w-[44px] h-[26px] rounded-full transition-all duration-200 flex-shrink-0 border ${
                      settings.auto_detect_profile !== "false"
                        ? "bg-primary border-primary shadow-sm shadow-primary/20"
                        : "bg-surface-container-high border-on-surface/[0.15]"
                    }`}
                    role="switch"
                    aria-checked={settings.auto_detect_profile !== "false"}
                  >
                    <span className={`absolute top-[3px] w-[20px] h-[20px] rounded-full bg-white shadow-md transition-transform duration-200 ${
                      settings.auto_detect_profile !== "false" ? "translate-x-[21px]" : "translate-x-[3px]"
                    }`} />
                  </button>
                </div>
                <div className="flex items-center justify-between py-2.5 min-h-[40px]">
                  <span className="text-sm text-on-surface">{t.transcription_input_lang}</span>
                  <div className="flex gap-1.5">
                    {['es', 'en'].map(lang => (
                      <button key={lang} onClick={() => updateSetting("language", lang)} className={`px-3 py-1 rounded-full text-[11px] font-black uppercase tracking-wider transition-all ${settings.language === lang ? 'bg-primary text-background shadow-sm' : 'bg-surface-container text-on-surface-variant border border-on-surface/[0.10] hover:bg-surface-container-high hover:text-on-surface'}`}>
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
              <button onClick={() => setConfirmModal(null)} className="px-4 py-2 rounded-xl bg-surface-container text-on-surface-variant text-[10px] font-black uppercase tracking-wider border border-on-surface/[0.10] hover:bg-surface-container-high transition-all">{t.cancel}</button>
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

// ── DICTIONARY SECTION ── tags view with inline edit
function DictionarySection({ dictionaryEntries, newWord, setNewWord, addWord, removeWord, updateReplacement, t }: {
  dictionaryEntries: any[];
  newWord: string;
  setNewWord: (v: string) => void;
  addWord: (w: string) => void;
  removeWord: (w: string) => void;
  updateReplacement: (word: string, replacement: string | null) => void;
  t: any;
}) {
  const [editingWord, setEditingWord] = useState<string | null>(null);
  const [editReplacement, setEditReplacement] = useState("");

  const openEdit = (entry: any) => {
    setEditingWord(entry.word);
    setEditReplacement(entry.replacement_word ?? "");
  };

  const saveEdit = (word: string) => {
    updateReplacement(word, editReplacement.trim() || null);
    setEditingWord(null);
  };

  return (
    <section>
      {/* Header */}
      <div className="flex items-baseline gap-2 mb-4">
        <h3 className="text-xl font-black text-on-surface font-headline">{t.personal_dictionary}</h3>
        {dictionaryEntries.length > 0 && (
          <span className="text-[10px] text-on-surface-variant font-mono">({dictionaryEntries.length})</span>
        )}
      </div>

      {/* Add word — at the TOP */}
      <div className="flex gap-2 mb-6">
        <input
          type="text"
          placeholder={t.dictionary_placeholder}
          value={newWord}
          onChange={(e) => setNewWord(e.target.value)}
          onKeyDown={(e) => { if (e.key === 'Enter' && newWord.trim()) { addWord(newWord.trim()); setNewWord(""); } }}
          className="flex-1 bg-surface-container rounded-xl px-4 py-2.5 text-sm text-on-surface border border-on-surface/[0.10] focus:outline-none focus:ring-2 focus:ring-primary/30 focus:border-primary/40 placeholder:text-on-surface-variant/60 transition-all"
        />
        <button
          onClick={() => { if (newWord.trim()) { addWord(newWord.trim()); setNewWord(""); } }}
          disabled={!newWord.trim()}
          className="px-5 bg-primary text-background rounded-xl text-[11px] font-black uppercase tracking-wider disabled:opacity-30 disabled:cursor-not-allowed hover:bg-primary/90 transition-all shadow-sm"
        >
          {t.add}
        </button>
      </div>

      {/* Empty state */}
      {dictionaryEntries.length === 0 ? (
        <div className="py-12 flex flex-col items-center gap-3 rounded-xl border-2 border-dashed border-on-surface/[0.08]">
          <span className="material-symbols-outlined text-3xl text-on-surface-variant/30">book</span>
          <p className="text-[11px] text-on-surface-variant font-semibold uppercase tracking-[0.2em]">{t.dictionary_empty}</p>
        </div>
      ) : (
        <div>
          {/* Tags cloud */}
          <div className="flex flex-wrap gap-2 mb-4">
            {dictionaryEntries.map(entry => (
              <div key={entry.word} className="group relative">
                <div
                  onClick={() => openEdit(entry)}
                  className={`flex items-center gap-1.5 px-3 py-1.5 rounded-full border cursor-pointer transition-all ${
                    editingWord === entry.word
                      ? 'bg-primary/10 border-primary/40 text-primary'
                      : 'bg-surface-container border-on-surface/[0.12] text-on-surface hover:bg-surface-container-high hover:border-primary/30'
                  }`}
                >
                  <span className="text-sm font-semibold">{entry.word}</span>
                  {entry.replacement_word && (
                    <span className="text-[10px] text-on-surface-variant">→ {entry.replacement_word}</span>
                  )}
                  <button
                    onClick={(e) => { e.stopPropagation(); removeWord(entry.word); }}
                    className="ml-0.5 w-4 h-4 flex items-center justify-center rounded-full opacity-0 group-hover:opacity-100 hover:bg-error/15 hover:text-error text-on-surface-variant transition-all"
                  >
                    <span className="material-symbols-outlined text-[12px]">close</span>
                  </button>
                </div>

                {/* Inline edit popover */}
                {editingWord === entry.word && (
                  <div className="absolute top-full left-0 mt-1 z-10 bg-surface-container-lowest rounded-xl shadow-lg border border-on-surface/[0.10] p-3 min-w-[220px] animate-in zoom-in-95 duration-150">
                    <p className="text-[9px] font-black uppercase tracking-widest text-on-surface-variant mb-2">
                      Corrección para <span className="text-on-surface">"{entry.word}"</span>
                    </p>
                    <div className="flex gap-2">
                      <input
                        autoFocus
                        type="text"
                        value={editReplacement}
                        onChange={(e) => setEditReplacement(e.target.value)}
                        onKeyDown={(e) => { if (e.key === 'Enter') saveEdit(entry.word); if (e.key === 'Escape') setEditingWord(null); }}
                        placeholder="Dejar vacío para quitar"
                        className="flex-1 bg-surface-container rounded-lg px-2.5 py-1.5 text-xs text-on-surface border border-on-surface/[0.10] focus:outline-none focus:ring-1 focus:ring-primary/30 placeholder:text-on-surface-variant/50"
                      />
                      <button
                        onClick={() => saveEdit(entry.word)}
                        className="px-3 py-1.5 bg-primary text-background rounded-lg text-[10px] font-black hover:bg-primary/90 transition-all"
                      >
                        OK
                      </button>
                    </div>
                  </div>
                )}
              </div>
            ))}
          </div>

          <p className="text-[10px] text-on-surface-variant/70 mt-2">
            Haz click en una palabra para definir una corrección automática. La × la elimina del diccionario.
          </p>
        </div>
      )}
    </section>
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
