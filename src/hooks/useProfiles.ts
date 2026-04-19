import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface Profile {
  id: number;
  name: string;
  system_prompt: string;
  icon: string | null;
  is_default: boolean;
}

export function useProfiles() {
  const [profiles, setProfiles] = useState<Profile[]>([]);

  useEffect(() => {
    invoke<Profile[]>("get_profiles")
      .then(setProfiles)
      .catch(console.error);

    const unlisten = listen("profiles-updated", () => {
      invoke<Profile[]>("get_profiles")
        .then(setProfiles)
        .catch(console.error);
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  return profiles;
}
