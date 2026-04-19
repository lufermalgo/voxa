import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

export type PipelineStatus = "idle" | "recording" | "processing" | "refining" | "loading_whisper" | "loading_llama" | "done";

export interface AppInfo {
  name: string;
  icon: string | null;
}

export interface ProfileDetectedEvent {
  name: string;
  is_auto: boolean;
}

export function useTranscription() {
  const [status, setStatus] = useState<PipelineStatus>("idle");
  const [rawText, setRawText] = useState("");
  const [refinedText, setRefinedText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [activeProfile, setActiveProfile] = useState<ProfileDetectedEvent | null>(null);

  useEffect(() => {
    const unlistenStatus = listen<string>("pipeline-status", (event) => {
      const s = event.payload as PipelineStatus;
      setStatus(s);
      if (s !== "recording") setAppInfo(null);
      if (s === "idle") setActiveProfile(null);
    });

    const unlistenRaw = listen<string>("pipeline-text-raw", (event) => {
      setRawText(event.payload);
    });

    const unlistenResults = listen<string>("pipeline-results", (event) => {
      setRefinedText(event.payload);
      setStatus("done");
      // Reset back to idle after a timeout
      setTimeout(() => setStatus("idle"), 3000);
    });

    const unlistenError = listen<string>("pipeline-error", (event) => {
      console.error("Pipeline Error:", event.payload);
      setError(event.payload);
      setStatus("idle");
      // Clear error after 5 seconds
      setTimeout(() => setError(null), 5000);
    });

    const unlistenAppDetected = listen<AppInfo>("app-detected", (event) => {
      setAppInfo(event.payload);
    });

    const unlistenProfileDetected = listen<ProfileDetectedEvent>("profile-detected", (event) => {
      setActiveProfile(event.payload);
    });

    return () => {
      unlistenStatus.then((f) => f());
      unlistenRaw.then((f) => f());
      unlistenResults.then((f) => f());
      unlistenError.then((f) => f());
      unlistenAppDetected.then((f) => f());
      unlistenProfileDetected.then((f) => f());
    };
  }, []);

  const downloadModels = async () => {
    try {
      await invoke("download_models");
    } catch (e) {
      setError(String(e));
    }
  };

  return { status, rawText, refinedText, error, appInfo, activeProfile, downloadModels };
}
