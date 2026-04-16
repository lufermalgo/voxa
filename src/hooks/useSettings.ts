import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface AppSettings {
  mic_id: string;
  language: string;
  interaction_mode: string;
  global_shortcut: string;
  shortcut_push_to_talk: string;
  shortcut_hands_free: string;
  shortcut_paste: string;
  shortcut_cancel: string;
  active_profile_id: string;
  auto_detect_profile: string;
}

export interface Profile {
  id: number;
  name: string;
  system_prompt: string;
  icon?: string;
  is_default: boolean;
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

    // Listen for updates from other windows/backend
    const unlistenSettings = import("@tauri-apps/api/event").then(({ listen }) =>
      listen("settings-updated", () => fetchSettings())
    );

    const unlistenProfiles = import("@tauri-apps/api/event").then(({ listen }) =>
      listen("profiles-updated", () => fetchSettings())
    );

    const unlistenDictionary = import("@tauri-apps/api/event").then(({ listen }) =>
      listen("dictionary-updated", async () => {
        const d = await invoke<string[]>("get_custom_dictionary");
        setDictionary(d);
      })
    );

    return () => {
      unlistenSettings.then(unlisten => unlisten());
      unlistenProfiles.then(unlisten => unlisten());
      unlistenDictionary.then(unlisten => unlisten());
    };
  }, [fetchSettings]);

  const updateSetting = async (key: keyof AppSettings, value: string) => {
    try {
      await invoke("update_setting", { key, value });
      // Update local state immediately for better UX
      setSettings((prev) => (prev ? { ...prev, [key]: value } : null));

      // If we just updated any shortcut, trigger Rust to apply them all
      if (key === "global_shortcut" || key.startsWith("shortcut_")) {
        await invoke("apply_all_shortcuts");
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

  const updateProfile = async (id: number, name: string, prompt: string, icon?: string) => {
    try {
      await invoke("update_profile", { id, name, prompt, icon });
      setProfiles(prev => prev.map(p => p.id === id ? { ...p, name, system_prompt: prompt, icon } : p));
    } catch (err: any) {
      setError(err?.toString() || "Error updating profile");
      await fetchSettings();
    }
  };

  const createProfile = async (name: string, prompt: string, icon?: string) => {
    try {
      await invoke("create_profile", { name, prompt, icon });
      await fetchSettings();
    } catch (err: any) {
      setError(err?.toString() || "Error creating profile");
    }
  };

  const deleteProfile = async (id: number) => {
    try {
      await invoke("delete_profile", { id });
      setProfiles(prev => prev.filter(p => p.id !== id));
    } catch (err: any) {
      setError(err?.toString() || "Error deleting profile");
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
    updateProfile,
    createProfile,
    deleteProfile,
    refresh: fetchSettings 
  };
}
