# Design: auto-profile-detection

## Overview

This document describes the technical design for automatic profile selection in Voxa. The feature detects the active app (or browser tab domain) at recording start and selects the appropriate transformation profile, with visual feedback in the RecorderPill and a quick-pick override accessible without opening Settings.

The design is intentionally minimal: it wires together existing infrastructure (the `ManualProfileOverride` state, `detect_profile_keyword_for_pid`, `FrontmostApp`, `SettingsCache`) rather than introducing new abstractions.

---

## Architecture

### Current state (before this feature)

```
StartRecording
  └─ capture FrontmostApp PID
  └─ emit "app-detected" (name + icon)

StopRecording
  └─ resolve_system_prompt()          ← profile detection happens HERE (wrong)
       └─ check ManualProfileOverride
       └─ detect_profile_keyword_for_pid(pid)
       └─ fallback to active_profile_id
```

### Target state (after this feature)

```
StartRecording
  └─ capture FrontmostApp PID
  └─ detect profile NOW (moved from StopRecording)
  └─ emit "app-detected"   (name + icon)
  └─ emit "profile-detected" (name, is_auto)   ← NEW

StopRecording
  └─ resolve_system_prompt()
       └─ reads pre-resolved profile from DetectedProfile state  ← reads cached result
       └─ no re-detection (profile was locked at start)
```

---

## Backend Changes (Rust)

### 1. New managed state: `DetectedProfile`

Add to `pipeline.rs`:

```rust
/// Profile resolved at StartRecording. Consumed by StopRecording.
/// Avoids re-detecting if the user switches apps mid-recording.
pub struct DetectedProfile(pub Mutex<Option<(String, String)>>); // (system_prompt, profile_name)
```

Register in `main.rs` alongside the other managed states:
```rust
.manage(pipeline::DetectedProfile(Mutex::new(None)))
```

### 2. Fix `domain_to_profile_keyword` — email inconsistency

In `pipeline.rs`, change the email mapping from `"Informal"` to `"Elegant"`:

```rust
// BEFORE (buggy):
if d == "mail.google.com" || d.contains("outlook.") ... { return Some("Informal"); }

// AFTER (correct):
fn domain_to_profile_keyword(domain: &str) -> Option<&'static str> {
    // Code contexts
    if d == "github.com" || d == "gitlab.com" || d.ends_with(".atlassian.net")
        || d == "linear.app" || d == "bitbucket.org" { return Some("Code"); }
    if d == "claude.ai" || d == "chat.openai.com" || d == "chatgpt.com" {
        return Some("Code");
    }
    // Informal / chat
    if d.ends_with(".slack.com") || d == "discord.com" || d == "twitter.com"
        || d == "x.com" { return Some("Informal"); }
    // Formal / writing — email is formal
    if d == "mail.google.com" || d.contains("outlook.") || d == "outlook.com"
        || d == "notion.so" || d == "docs.google.com" || d == "coda.io"
        || d.contains("confluence") { return Some("Elegant"); }
    None
}
```

Also add missing mappings to `bundle_id_to_profile_keyword`:
- `"com.todesktop.230313mzl4w4u92"` (Cursor) → already present ✓
- No new bundle IDs needed beyond what requirements specify.

### 3. Move profile detection to `StartRecording`

In `pipeline.rs`, inside the `DictationEvent::StartRecording` arm, after capturing `FrontmostApp`:

```rust
// After capturing FrontmostApp PID and emitting "app-detected":

// Resolve and cache the profile for this recording session
let resolved = resolve_system_prompt(&app, &db_state);
let is_auto = app.state::<ManualProfileOverride>().0.lock().unwrap().is_none()
    && app.state::<db::SettingsCache>().get("auto_detect_profile")
        .map(|v| v != "false").unwrap_or(true);

let _ = app.emit("profile-detected", serde_json::json!({
    "name": resolved.1,
    "is_auto": is_auto,
}));

*app.state::<DetectedProfile>().0.lock().unwrap() = Some(resolved);
```

### 4. Update `StopRecording` to consume cached profile

In the `DictationEvent::StopRecording` arm, replace the two `resolve_system_prompt` calls with reads from `DetectedProfile`:

```rust
// BEFORE:
let (system_prompt, profile_name) = resolve_system_prompt(&app, &db_state);

// AFTER:
let (system_prompt, profile_name) = app
    .state::<DetectedProfile>()
    .0
    .lock()
    .unwrap()
    .clone()
    .unwrap_or_else(|| resolve_system_prompt(&app, &db_state)); // fallback if state missing
```

Clear `DetectedProfile` after use (at end of `StopRecording` and `CancelRecording`):
```rust
*app.state::<DetectedProfile>().0.lock().unwrap() = None;
```

### 5. New Tauri command: `set_manual_profile_override`

Add to `main.rs` (or a commands module):

```rust
#[tauri::command]
fn set_manual_profile_override(
    app: tauri::AppHandle,
    profile_name: Option<String>, // None = clear override (back to auto)
) {
    *app.state::<pipeline::ManualProfileOverride>().0.lock().unwrap() = profile_name;
}
```

### 6. New Tauri command: `get_profiles_for_picker`

Reuse the existing `get_profiles` db function, exposed as a command (may already exist — check `main.rs`). If not:

```rust
#[tauri::command]
fn get_profiles_for_picker(db: tauri::State<db::DbState>) -> Vec<db::Profile> {
    let conn = db.conn.lock().unwrap();
    db::get_profiles(&conn).unwrap_or_default()
}
```

### 7. Performance: URL detection timeout

`get_browser_tab_url` is called synchronously inside `detect_profile_keyword_for_pid`. To enforce the 50ms timeout from Requirement 8.3, wrap the call with a thread + channel:

```rust
// In detect_profile_keyword_for_pid, replace the direct call:
let url = std::thread::scope(|s| {
    let handle = s.spawn(|| crate::event_tap::get_browser_tab_url(pid, &bundle_id));
    // Wait up to 50ms
    // Note: thread::scope doesn't support timeout natively; use a channel instead:
});

// Practical implementation using mpsc:
let (tx, rx) = std::sync::mpsc::channel();
let bid_clone = bundle_id.clone();
std::thread::spawn(move || {
    let result = crate::event_tap::get_browser_tab_url(pid, &bid_clone);
    let _ = tx.send(result);
});
let url = rx.recv_timeout(std::time::Duration::from_millis(50)).ok().flatten();
```

---

## Frontend Changes (TypeScript/React)

### 1. New event type in `useTranscription.ts`

Add `ProfileDetectedEvent` and listen for `"profile-detected"`:

```typescript
export interface ProfileDetectedEvent {
  name: string;
  is_auto: boolean;
}

// In useTranscription hook, add state:
const [activeProfile, setActiveProfile] = useState<ProfileDetectedEvent | null>(null);

// In useEffect, add listener:
const unlistenProfile = listen<ProfileDetectedEvent>("profile-detected", (event) => {
  setActiveProfile(event.payload);
});

// Clear on idle:
if (s === "idle") setActiveProfile(null);

// Return from hook:
return { status, rawText, refinedText, error, appInfo, activeProfile, downloadModels };
```

### 2. New component: `ProfilePill`

A small badge rendered inside `RecorderPill` during `recording` and `idle` states.

**File:** `src/components/ProfilePill.tsx`

```tsx
interface ProfilePillProps {
  profileName: string;
  isAuto: boolean;
  onClick: () => void;
}

export const ProfilePill = ({ profileName, isAuto, onClick }: ProfilePillProps) => {
  const displayName = profileName.length > 8
    ? profileName.slice(0, 7) + "…"
    : profileName;

  return (
    <button
      onClick={onClick}
      className="flex items-center gap-1 px-2 py-0.5 rounded-full bg-white/10 hover:bg-white/20 transition-colors text-white/80 hover:text-white text-[10px] font-bold uppercase tracking-wider flex-shrink-0 z-10"
    >
      {isAuto && (
        <span className="material-symbols-outlined !text-[10px] text-primary/80">auto_awesome</span>
      )}
      <span>{displayName}</span>
    </button>
  );
};
```

### 3. New component: `ProfilePicker`

A floating popover that lists all profiles. Appears above the pill on click.

**File:** `src/components/ProfilePicker.tsx`

```tsx
interface ProfilePickerProps {
  profiles: Profile[];
  currentProfileName: string;
  onSelect: (profileName: string | null) => void; // null = clear override (Auto)
  onClose: () => void;
}

export const ProfilePicker = ({ profiles, currentProfileName, onSelect, onClose }: ProfilePickerProps) => {
  // Click-outside to close
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [onClose]);

  return (
    <div
      ref={ref}
      className="absolute bottom-[calc(100%+8px)] left-1/2 -translate-x-1/2 z-50
                 bg-[#0A0A0A]/90 backdrop-blur-[40px] border border-white/10
                 rounded-[16px] shadow-[0_20px_60px_rgba(0,0,0,0.6)]
                 p-2 min-w-[160px] animate-in fade-in slide-in-from-bottom-2 duration-200"
    >
      {/* Auto option — clears manual override */}
      <button
        onClick={() => { onSelect(null); onClose(); }}
        className="w-full flex items-center gap-2 px-3 py-2 rounded-xl
                   text-[11px] font-bold text-white/60 hover:text-white
                   hover:bg-white/10 transition-colors"
      >
        <span className="material-symbols-outlined !text-[14px]">auto_awesome</span>
        Auto
      </button>

      <div className="h-px bg-white/10 my-1" />

      {profiles.map(profile => (
        <button
          key={profile.id}
          onClick={() => { onSelect(profile.name); onClose(); }}
          className={`w-full flex items-center gap-2 px-3 py-2 rounded-xl
                      text-[11px] font-bold transition-colors
                      ${profile.name === currentProfileName
                        ? "text-white bg-white/10"
                        : "text-white/60 hover:text-white hover:bg-white/10"}`}
        >
          <span className="material-symbols-outlined !text-[14px] material-symbols-fill">
            {profile.icon || "psychology"}
          </span>
          {profile.name}
        </button>
      ))}
    </div>
  );
};
```

### 4. Update `RecorderPill.tsx`

Add `ProfilePill` and `ProfilePicker` to the pill layout.

**Props additions:**
```typescript
interface RecorderPillProps {
  // ... existing props
  activeProfile?: ProfileDetectedEvent | null;
  profiles?: Profile[];
}
```

**Inside the recording section**, add `ProfilePill` between the waveform bars and the app icon:
```tsx
{isRecording && activeProfile && (
  <ProfilePill
    profileName={activeProfile.name}
    isAuto={activeProfile.is_auto}
    onClick={() => setPickerOpen(true)}
  />
)}
```

**Inside the idle section** (new — currently the pill is invisible when idle, but the pill state machine allows it):
- The pill is `pointer-events-none` when idle, so the profile pill click only works during recording. This is acceptable for v1 — the override is most useful during an active recording.

**Picker rendering** (above the pill, same pattern as the warning card):
```tsx
{pickerOpen && (
  <div className="absolute bottom-[calc(100%+8px)] ...">
    <ProfilePicker
      profiles={profiles}
      currentProfileName={activeProfile?.name ?? ""}
      onSelect={async (name) => {
        await invoke("set_manual_profile_override", { profileName: name });
        setPickerOpen(false);
      }}
      onClose={() => setPickerOpen(false)}
    />
  </div>
)}
```

### 5. Update `App.tsx`

Pass `activeProfile` and `profiles` down to `RecorderPill`:

```tsx
const { status, appInfo, activeProfile } = useTranscription();
const { profiles } = useSettings(); // already available or add a lightweight fetch

<RecorderPill
  status={...}
  uiLocale={uiLocale}
  appInfo={appInfo}
  activeProfile={activeProfile}
  profiles={profiles}
/>
```

> **Note on profiles in App.tsx**: `useSettings` is currently only used in `SettingsPanel`. To avoid loading the full settings hook in the main pill window, add a minimal `useProfiles` hook that only fetches `get_profiles` once on mount. This keeps the pill window lightweight.

### 6. New hook: `useProfiles`

**File:** `src/hooks/useProfiles.ts`

```typescript
export function useProfiles() {
  const [profiles, setProfiles] = useState<Profile[]>([]);

  useEffect(() => {
    invoke<Profile[]>("get_profiles").then(setProfiles).catch(console.error);

    // Refresh if profiles change (e.g., user creates one in Settings)
    const unlisten = listen("profiles-updated", () => {
      invoke<Profile[]>("get_profiles").then(setProfiles).catch(console.error);
    });
    return () => { unlisten.then(f => f()); };
  }, []);

  return profiles;
}
```

---

## Data Flow Diagram

```
User presses shortcut
        │
        ▼
event_tap.rs → DictationEvent::StartRecording
        │
        ▼
pipeline.rs: StartRecording handler
  1. capture FrontmostApp (PID, name, icon)
  2. resolve_system_prompt()
     ├─ ManualProfileOverride? → use it
     ├─ auto_detect_profile=true? → detect_profile_keyword_for_pid(pid)
     │    ├─ bundle_id_to_profile_keyword()
     │    └─ domain_to_profile_keyword() [with 50ms timeout]
     └─ fallback: active_profile_id from DB
  3. store result in DetectedProfile state
  4. emit "app-detected"     → frontend shows app icon
  5. emit "profile-detected" → frontend shows ProfilePill
        │
        ▼
User speaks, releases shortcut
        │
        ▼
pipeline.rs: StopRecording handler
  1. read DetectedProfile (no re-detection)
  2. Whisper transcription
  3. LLM refinement with cached system_prompt
  4. paste result
  5. clear DetectedProfile
```

---

## State Management

| State | Location | Lifetime | Purpose |
|-------|----------|----------|---------|
| `ManualProfileOverride` | Rust `Mutex<Option<String>>` | Session (cleared on restart) | User's explicit profile choice |
| `DetectedProfile` | Rust `Mutex<Option<(String,String)>>` | Per-recording | Cached profile locked at StartRecording |
| `FrontmostApp` | Rust `Mutex<AppInfo>` | Per-recording | App that was active when recording started |
| `activeProfile` | React `useState` | While recording | Drives ProfilePill display |
| `pickerOpen` | React `useState` (in RecorderPill) | Transient | Controls ProfilePicker visibility |

---

## Profile Mapping Reference

### Bundle ID → Profile

| Bundle ID | Profile |
|-----------|---------|
| `com.microsoft.vscode` | Code |
| `com.todesktop.230313mzl4w4u92` (Cursor) | Code |
| `com.apple.dt.xcode` | Code |
| `com.jetbrains.*` | Code |
| `com.tinyspeck.slackmacgap` | Informal |
| `com.hnc.discord` | Informal |
| `com.microsoft.teams2` | Informal |
| `ru.keepcoder.telegram` | Informal |
| `com.apple.mail` | Elegant |
| `com.microsoft.outlook` | Elegant |
| `notion.id` | Elegant |
| `com.apple.notes` | Elegant |
| `md.obsidian` | Elegant |
| `com.evernote.evernote` | Elegant |

### Domain → Profile

| Domain pattern | Profile |
|----------------|---------|
| `github.com`, `gitlab.com`, `linear.app`, `bitbucket.org`, `*.atlassian.net` | Code |
| `claude.ai`, `chat.openai.com`, `chatgpt.com` | Code |
| `*.slack.com`, `discord.com`, `twitter.com`, `x.com` | Informal |
| `mail.google.com`, `outlook.com`, `outlook.*` | Elegant |
| `notion.so`, `docs.google.com`, `coda.io`, `*confluence*` | Elegant |

---

## Files to Modify

### Rust (src-tauri/src/)

| File | Change |
|------|--------|
| `pipeline.rs` | Add `DetectedProfile` state; fix `domain_to_profile_keyword`; move detection to `StartRecording`; update `StopRecording` to read cached profile; add 50ms timeout for URL detection |
| `main.rs` | Register `DetectedProfile` state; add `set_manual_profile_override` command; expose `get_profiles_for_picker` if not already a command |

### TypeScript (src/)

| File | Change |
|------|--------|
| `hooks/useTranscription.ts` | Add `ProfileDetectedEvent` type; listen for `"profile-detected"`; expose `activeProfile` |
| `hooks/useProfiles.ts` | **New file** — lightweight hook to fetch profiles list |
| `components/ProfilePill.tsx` | **New file** — badge showing active profile name + auto indicator |
| `components/ProfilePicker.tsx` | **New file** — floating popover with profile list + Auto option |
| `components/RecorderPill.tsx` | Add `ProfilePill` inside recording state; add `ProfilePicker` popover; add `pickerOpen` state |
| `App.tsx` | Pass `activeProfile` and `profiles` to `RecorderPill` |

---

## Edge Cases and Error Handling

**Profile deleted mid-session**: If `ManualProfileOverride` holds a profile name that no longer exists in the DB, `resolve_system_prompt` falls through to the `active_profile_id` fallback. No crash.

**URL detection timeout**: If `get_browser_tab_url` exceeds 50ms, the channel `recv_timeout` returns `Err`, and the code falls back to bundle-ID matching or the default profile. The recording is not delayed.

**Accessibility permissions denied**: `get_browser_tab_url` already returns `None` gracefully on permission errors. The fallback chain handles this transparently.

**No profile match**: Falls back to `active_profile_id` from Settings (the user's manually selected default).

**Recording cancelled**: `CancelRecording` clears `DetectedProfile` so the next recording starts fresh.

**Picker open during recording**: Selecting a profile calls `set_manual_profile_override` on the Rust side. This updates `ManualProfileOverride` but does NOT change `DetectedProfile` for the current recording — the current recording uses the profile that was locked at start. The new override takes effect on the next recording. This is the correct behavior (changing mid-recording would be confusing).

> **Design decision**: Should selecting a profile in the picker during an active recording affect the current recording? The requirements (Req 4.5) say yes: "THE Profile_Detector SHALL use that profile for the recording in cours." To support this, `set_manual_profile_override` should also update `DetectedProfile` when a recording is active. This requires checking `RecordingState` in the command and updating both states atomically.

---

## Correctness Properties

### Property 1: Detection timing invariant
For any recording session, the profile used for LLM refinement MUST be the profile that was resolved at `StartRecording`, not at `StopRecording`. Verifiable by: recording in VS Code, switching to Slack before stopping — the output should use the Code profile.

### Property 2: Email mapping consistency
`mail.google.com`, `com.apple.mail`, and `com.microsoft.outlook` MUST all resolve to the same profile ("Elegant"). No email context should resolve to "Informal".

### Property 3: Manual override priority
When `ManualProfileOverride` is set, `resolve_system_prompt` MUST return that profile regardless of the active app's bundle ID or domain.

### Property 4: Override session scope
After Voxa restarts, `ManualProfileOverride` MUST be `None` (auto-detection resumes). This is guaranteed by the `Mutex::new(None)` initialization in `main.rs`.

### Property 5: Fallback completeness
`resolve_system_prompt` MUST always return a non-empty system prompt. The fallback chain (override → auto-detect → active_profile_id → first profile) ensures this as long as at least one profile exists in the DB.

### Property 6: URL timeout
`detect_profile_keyword_for_pid` MUST complete within 100ms total (Req 8.1). The 50ms timeout on URL detection leaves 50ms for bundle ID lookup and DB query, which are both in-memory operations.

### Property 7: ProfilePill truncation
Profile names longer than 8 characters MUST be displayed truncated with "…" in the ProfilePill. Names of 8 characters or fewer MUST be displayed in full.

### Property 8: Picker closes on outside click
Clicking outside the `ProfilePicker` popover MUST close it without changing the active profile.
