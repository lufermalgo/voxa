import { useTranscription } from "./hooks/useTranscription";
import { RecorderPill } from "./components/RecorderPill";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { SettingsPanel } from "./components/SettingsPanel";
import { TrayMenu } from "./components/TrayMenu";
import "./App.css";

function App() {
  const { status, rawText, refinedText, error, downloadModels } = useTranscription();
  const [modelsMissing, setModelsMissing] = useState(false);
  const [windowLabel, setWindowLabel] = useState<string>(() => getCurrentWindow().label);
  const [activeTab, setActiveTab] = useState<string>("general");
  const [isOnboarded, setIsOnboarded] = useState<boolean>(true);

  useEffect(() => {
    const label = getCurrentWindow().label;
    if (label !== windowLabel) setWindowLabel(label);

    const init = async () => {
      try {
        const missing = await invoke<boolean>("check_models_status");
        setModelsMissing(missing);
        
        const settings = await invoke<Record<string, string>>("get_settings");
        setIsOnboarded(settings.is_onboarded === "true");
      } catch (err) {
        console.error("Initialization error:", err);
      }
    };

    init();

    const unlisten = listen<string>("show-tab", (event) => {
      setActiveTab(event.payload);
    });

    return () => {
      unlisten.then(f => f());
    };
  }, []);

  // Settings Window
  if (windowLabel === "settings") {
    return (
      <div className="h-screen w-screen bg-[#131314] text-white overflow-hidden">
        <SettingsPanel 
          onClose={() => getCurrentWindow().hide()} 
          initialTab={activeTab}
        />
      </div>
    );
  }

  // Tray Menu Window (Hidden by default, shown on tray click)
  if (windowLabel === "tray_menu") {
    return (
      <div className="h-screen w-screen bg-transparent overflow-hidden">
        <TrayMenu />
      </div>
    );
  }

  // Floating Pill Window (Main)
  if (windowLabel === "main") {
    return (
      <div className="w-full h-full flex items-end justify-center pb-[10px] bg-transparent overflow-hidden">
        <RecorderPill status={modelsMissing ? "loading" : status} />
      </div>
    );
  }

  // Fallback - Don't render anything until we have a label to avoid centering bugs
  return null;
}

export default App;
