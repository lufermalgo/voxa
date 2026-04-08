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
  const { status } = useTranscription();
  const [windowLabel, setWindowLabel] = useState<string>(() => getCurrentWindow().label);
  const [activeTab, setActiveTab] = useState<string>("general");
  const [isDownloading, setIsDownloading] = useState(false);
  const [downloadStatus, setDownloadStatus] = useState("");
  const [uiLocale, setUiLocale] = useState<Locale>("en");

  const fetchSettings = async (): Promise<Locale> => {
    try {
      await invoke("get_settings");
      const systemLocale = await invoke<string>("get_system_locale");
      const locale: Locale = systemLocale.startsWith("es") ? "es" : "en";
      setUiLocale(locale);
      return locale;
    } catch (err) {
      console.error("Error fetching settings:", err);
      return "es";
    }
  };

  useEffect(() => {
    const label = getCurrentWindow().label;
    if (label !== windowLabel) setWindowLabel(label);

    const init = async () => {
      try {
        const exists = await invoke<boolean>("check_models_status");
        const locale = await fetchSettings();

        if (!exists) {
          const t = translations[locale];
          const label = getCurrentWindow().label;

          if (label === "main") {
            setIsDownloading(true);
            setDownloadStatus(locale === "es" ? "Proveyendo IA (Metal)..." : "Provisioning AI (Metal)...");
            try {
              await invoke("download_models");
              setDownloadStatus(locale === "es" ? "Listo" : "Ready");
              setIsDownloading(false);
            } catch (error) {
              console.error("Error downloading models:", error);
              setDownloadStatus(locale === "es" ? "Error. Revisar logs." : "Error. Check logs.");
            }
          } else {
            setIsDownloading(true);
            setDownloadStatus(t.downloading_model.replace("{model}", "AI").replace("{progress}", "0"));
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
       setIsDownloading(true);
       setDownloadStatus(t.downloading_model.replace("{model}", event.payload.model).replace("{progress}", Math.round(event.payload.progress).toString()));
    });

    const unlistenComplete = listen("download-complete", () => {
      setIsDownloading(false);
      setDownloadStatus("");
    });

    return () => {
      unlistenStatus.then(f => f());
      unlistenSettings.then(f => f());
      unlistenProgress.then(f => f());
      unlistenComplete.then(f => f());
    };
  }, []);

  // Settings Window
  if (windowLabel === "settings") {
    return (
      <div className="h-screen w-screen bg-background text-on-surface overflow-hidden">
        <SettingsPanel 
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
