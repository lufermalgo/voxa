import { useState, useEffect, useRef, useLayoutEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { t, Locale } from "../i18n";

export const TrayMenu = () => {
  const [language, setLanguage] = useState("es");
  const [devices, setDevices] = useState<any[]>([]);
  const [micId, setMicId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const containerRef = useRef<HTMLDivElement>(null);
  
  // UI State for toggling sections
  const [expandedSection, setExpandedSection] = useState<"mic" | "lang" | null>(null);

  // Auto-resize window to fit content
  useLayoutEffect(() => {
    const resizeWindow = async () => {
      if (containerRef.current) {
        // Measure the actual height of the content
        const height = containerRef.current.scrollHeight;
        const width = 240; // Fixed width for consistency
        const appWindow = getCurrentWindow();
        await appWindow.setSize(new LogicalSize(width, height));
      }
    };

    // Resize on mount and potential content changes
    resizeWindow();
    
    // Also resize when language or mic changes (since toggle states might shift layout)
    const timer = setTimeout(resizeWindow, 50); 
    return () => clearTimeout(timer);
  }, [language, micId, expandedSection]);

  useEffect(() => {
    const init = async () => {
      try {
        const [allSettings, allDevices] = await Promise.all([
          invoke<Record<string, string>>("get_settings"),
          invoke<any[]>("get_audio_devices")
        ]);
        
        setLanguage(allSettings.language || "es");
        setDevices(allDevices);
        setMicId(allSettings.mic_id || null);
      } catch (err) {
        console.error("TrayMenu init error:", err);
      } finally {
        setLoading(false);
      }
    };

    init();

    const unlistenSettings = getCurrentWindow().listen("settings-updated", () => {
      init();
    });

    return () => {
      unlistenSettings.then(f => f());
    };
  }, []);

  const closeMenu = () => getCurrentWindow().hide();
  
  const handleUpdateSetting = async (key: string, value: string) => {
    try {
      await invoke("update_setting", { key, value });
      if (key === "language") setLanguage(value);
      if (key === "mic_id") setMicId(value);
      // Close expansion after selection to keep it compact
      setExpandedSection(null);
    } catch (err) {
      console.error(`Error updating setting ${key}:`, err);
    }
  };

  const openSettings = (tab?: string) => {
    invoke("show_settings", { tab });
    closeMenu();
  };

  const handleQuit = () => invoke("exit_app");

  if (loading) return null;

  return (
    <div 
      ref={containerRef}
      className="w-[240px] h-fit tray-menu-container native-vibrancy flex flex-col overflow-hidden text-white select-none animate-in fade-in zoom-in-95 duration-200 border-[0.5px] border-white/20 rounded-[12px]"
    >
      <div className="p-1 flex flex-col">
        {/* Microfono Section */}
        <div className="rounded-[6px] overflow-hidden transition-all duration-300">
          <ActionItem
            icon="settings_voice"
            label={t(language as Locale, "tray_mic")}
            value={micId === "auto" || !micId ? "Auto" : (devices.find(d => d.name === micId)?.name || micId).slice(0, 11)}
            onClick={() => setExpandedSection(expandedSection === "mic" ? null : "mic")}
            isActive={expandedSection === "mic"}
          />
          {expandedSection === "mic" && (
            <div className="mx-1 mb-1 p-0.5 bg-black/5 dark:bg-white/5 rounded-[4px] animate-in slide-in-from-top-1 duration-200 max-h-[140px] overflow-y-auto custom-scrollbar">
              <SubItem
                label={t(language as Locale, "tray_auto_detect")}
                isActive={!micId || micId === 'auto'}
                onClick={() => handleUpdateSetting("mic_id", "auto")}
              />
              {devices.map((d) => (
                <SubItem
                  key={d.name}
                  label={d.name}
                  isActive={micId === d.name}
                  onClick={() => handleUpdateSetting("mic_id", d.name)}
                />
              ))}
            </div>
          )}
        </div>

        {/* Lenguaje Section */}
        <div className="rounded-[6px] overflow-hidden transition-all duration-300">
          <ActionItem
            icon="language"
            label={t(language as Locale, "tray_lang")}
            value={language.toUpperCase() === "ES" ? "ES" : "EN"}
            onClick={() => setExpandedSection(expandedSection === "lang" ? null : "lang")}
            isActive={expandedSection === "lang"}
          />
          {expandedSection === "lang" && (
            <div className="mx-1 mb-1 p-1 bg-black/5 dark:bg-white/5 rounded-[4px] animate-in slide-in-from-top-1 duration-200 flex gap-1">
              <button
                onClick={() => handleUpdateSetting("language", "es")}
                className={`flex-1 py-1 rounded-md text-[10px] font-bold transition-all px-2 ${language.toLowerCase() === 'es' ? 'bg-[#007AFF] text-white shadow-sm' : 'text-white/40 hover:text-white/90 hover:bg-white/5'}`}
              >
                ESPAÑOL
              </button>
              <button
                onClick={() => handleUpdateSetting("language", "en")}
                className={`flex-1 py-1 rounded-md text-[10px] font-bold transition-all px-2 ${language.toLowerCase() === 'en' ? 'bg-[#007AFF] text-white shadow-sm' : 'text-white/40 hover:text-white/90 hover:bg-white/5'}`}
              >
                ENGLISH
              </button>
            </div>
          )}
        </div>

        <div className="tray-divider" />

        {/* Action Items */}
        <div className="space-y-0.5">
          <ActionItem icon="history" label={t(language as Locale, "tray_history")} onClick={() => openSettings('history')} />
          <ActionItem icon="tune" label={t(language as Locale, "tray_profiles")} onClick={() => openSettings('profiles')} />
          <ActionItem icon="spellcheck" label={t(language as Locale, "tray_dictionary")} onClick={() => openSettings('dictionary')} />

          <div className="tray-divider" />

          <ActionItem icon="settings" label={t(language as Locale, "tray_settings")} onClick={() => openSettings()} />
          <ActionItem icon="help" label={t(language as Locale, "tray_help")} onClick={() => {}} />

          <div className="tray-divider" />

          <ActionItem icon="power_settings_new" label={t(language as Locale, "tray_quit")} onClick={handleQuit} isDestructive />
        </div>
      </div>
    </div>
  );
};

const ActionItem = ({ icon, label, value, onClick, isDestructive, isActive }: any) => (
  <button 
    onClick={onClick}
    className={`w-full flex items-center justify-between px-2.5 py-1.5 rounded-[5px] transition-all group shrink-0 menu-item-hover
      ${isActive ? 'bg-[#007AFF] text-white' : 'text-white/90 hover:text-white dark:text-white/90'}`}
  >
    <div className="flex items-center space-x-2.5">
      <span className={`material-symbols-outlined tray-icon ${isActive ? 'text-white' : 'opacity-70 group-hover:opacity-100 transition-all'}`}>{icon}</span>
      <span className="text-[13px] font-normal leading-none tracking-tight">{label}</span>
    </div>
    {value && !isDestructive && !isActive && (
      <span className="text-[10px] font-medium opacity-40 group-hover:opacity-100 px-1.5 py-0.5 rounded-sm transition-all uppercase tracking-tight">{value}</span>
    )}
  </button>
);

const SubItem = ({ label, isActive, onClick }: any) => (
  <button 
    onClick={onClick}
    className={`w-full flex items-center justify-between px-2 py-1 rounded-[3px] transition-all text-left group
      ${isActive ? 'bg-[#007AFF] text-white' : 'hover:bg-white/10 text-white/50 hover:text-white/90'}`}
  >
    <span className="text-[12px] font-normal truncate pr-2">{label}</span>
    {isActive && <span className="material-symbols-outlined text-[13px] text-white">check</span>}
  </button>
);

