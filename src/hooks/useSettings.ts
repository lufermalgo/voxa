import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface AppSettings {
  mic_id: string;
  language: string;
  interaction_mode: string;
  global_shortcut: string;
  active_profile_id: string;
}

export interface Profile {
  id: number;
  name: string;
  system_prompt: string;
}

export function useSettings() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [dictionary, setDictionary] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchSettings = useCallback(async () => {
    try {
      setLoading(true);
      const [s, p, d] = await Promise.all([
        invoke<AppSettings>("get_settings"),
        invoke<Profile[]>("get_profiles"),
        invoke<string[]>("get_custom_dictionary")
      ]);
      setSettings(s);
      setProfiles(p);
      setDictionary(d);
      setError(null);
    } catch (err: any) {
      console.error("Failed to load settings:", err);
      setError(err?.toString() || "Unknown error loading settings");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchSettings();
  }, [fetchSettings]);

  const updateSetting = async (key: keyof AppSettings, value: string) => {
    try {
      await invoke("update_setting", { key, value });
      // Update local state immediately for better UX
      setSettings((prev) => (prev ? { ...prev, [key]: value } : null));

      // If we just updated the global_shortcut, we might want to register it via Rust
      // The Rust side should probably handle unregistering the old and registering the new shortcut
      // Wait, let's trigger a Rust command to apply the new shortcut
      if (key === "global_shortcut") {
        await invoke("apply_shortcut", { shortcut: value });
      }
    } catch (err: any) {
      console.error(`Failed to update ${key}:`, err);
      setError(err?.toString() || `Error updating ${key}`);
      // Refresh to get actual db state in case of failure
      await fetchSettings();
    }
  };

  const addWord = async (word: string) => {
    try {
      await invoke("add_to_dictionary", { word });
      setDictionary(prev => [...prev.filter(w => w !== word), word]);
    } catch (err: any) {
      setError(err?.toString() || "Error adding word");
    }
  };

  const removeWord = async (word: string) => {
    try {
      await invoke("remove_from_dictionary", { word });
      setDictionary(prev => prev.filter(w => w !== word));
    } catch (err: any) {
      setError(err?.toString() || "Error removing word");
    }
  };

  return { 
    settings, 
    profiles, 
    dictionary, 
    loading, 
    error, 
    updateSetting, 
    addWord, 
    removeWord, 
    refresh: fetchSettings 
  };
}
