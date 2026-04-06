import { useTranscription } from "./hooks/useTranscription";
import { RecorderPill } from "./components/RecorderPill";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { SettingsPanel } from "./components/SettingsPanel";
import { Locale, translations } from "./i18n";

import "./App.css";

function App() {
  const { status, rawText, refinedText, error, downloadModels } = useTranscription();
  const [hasModels, setHasModels] = useState(true);
  const [windowLabel, setWindowLabel] = useState<string>(() => getCurrentWindow().label);
  const [activeTab, setActiveTab] = useState<string>("general");
  const [isDownloading, setIsDownloading] = useState(false);
  const [downloadStatus, setDownloadStatus] = useState("");
  const [uiLocale, setUiLocale] = useState<Locale>("en");

  const fetchSettings = async () => {
    try {
      await invoke("get_settings");
      
      const systemLocale = await invoke<string>("get_system_locale");
      setUiLocale(systemLocale.startsWith("es") ? "es" : "en");
    } catch (err) {
      console.error("Error fetching settings:", err);
    }
  };

  useEffect(() => {
    const label = getCurrentWindow().label;
    if (label !== windowLabel) setWindowLabel(label);

    const init = async () => {
      try {
        const exists = await invoke<boolean>("check_models_status");
        setHasModels(exists);
        
        await fetchSettings();

        if (!exists) {
          const label = getCurrentWindow().label;
          const isEs = (await invoke<string>("get_system_locale")).startsWith("es");

          if (label === "main") {
            setIsDownloading(true);
            setDownloadStatus(isEs ? "Proveyendo IA (Metal)..." : "Provisioning AI (Metal)...");
            try {
              await invoke("download_models");
              setDownloadStatus(isEs ? "Listo" : "Ready");
              setIsDownloading(false);
              setHasModels(true);
            } catch (error) {
              console.error("Error downloading models:", error);
              setDownloadStatus(isEs ? "Error. Revisar logs." : "Error. Check logs.");
            }
          } else {
            setIsDownloading(true);
            setDownloadStatus(isEs ? "Descargando IA..." : "Downloading AI...");
          }
        }
      } catch (err) {
        console.error("Initialization error:", err);
      }
    };

    init();

    const unlistenStatus = listen<string>("show-tab", (event) => {
      setActiveTab(event.payload);
    });

    const unlistenSettings = listen("settings-updated", () => {
      fetchSettings();
    });

    const unlistenProgress = listen<{model: string, progress: number}>("download-progress", (event) => {
       const t = translations[uiLocale];
       setDownloadStatus(t.downloading_model.replace("{model}", event.payload.model).replace("{progress}", Math.round(event.payload.progress).toString()));
    });

    return () => {
      unlistenStatus.then(f => f());
      unlistenSettings.then(f => f());
      unlistenProgress.then(f => f());
    };
  }, []);

  // Settings Window
  if (windowLabel === "settings") {
    return (
      <div className="h-screen w-screen bg-background text-on-surface overflow-hidden">
        <SettingsPanel 
          onClose={() => getCurrentWindow().hide()} 
          initialTab={activeTab}
          uiLocale={uiLocale}
        />
      </div>
    );
  }

  // Floating Pill Window (Main)
  if (windowLabel === "main") {
    return (
      <div className="w-full h-full flex items-end justify-center pb-5 bg-transparent overflow-hidden">
        <RecorderPill 
          status={isDownloading ? "loading" : status} 
          label={isDownloading ? downloadStatus : undefined} 
          uiLocale={uiLocale}
        />
      </div>
    );
  }

  // Fallback - Don't render anything until we have a label to avoid centering bugs
  return null;
}

export default App;
