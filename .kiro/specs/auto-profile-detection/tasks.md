# Implementation Plan: auto-profile-detection

## Overview

Wire together the existing Rust detection infrastructure with the correct timing, fix the email mapping inconsistency, add the `profile-detected` event, and build the ProfilePill + ProfilePicker UI components. The implementation follows the data flow: backend detection at `StartRecording` → event emission → frontend state → ProfilePill display → ProfilePicker override.

## Tasks

- [x] 1. Fix Rust backend: `DetectedProfile` state and email mapping
  - [x] 1.1 Add `DetectedProfile` managed state to `pipeline.rs`
    - Define `pub struct DetectedProfile(pub Mutex<Option<(String, String)>>)` in `pipeline.rs`
    - Register `.manage(pipeline::DetectedProfile(Mutex::new(None)))` in `main.rs`
    - _Requirements: 1.1, 9.1_

  - [ ]* 1.2 Write property test for `DetectedProfile` initialization
    - **Property 4: Override session scope** — after initialization, `DetectedProfile` MUST be `None`
    - **Validates: Requirements 5.2**

  - [x] 1.3 Fix `domain_to_profile_keyword` email mapping inconsistency
    - Change `mail.google.com` mapping from `"Informal"` to `"Elegant"`
    - Ensure `outlook.com` and `outlook.*` domains map to `"Elegant"`
    - Ensure `docs.google.com`, `notion.so`, `coda.io`, `*confluence*` map to `"Elegant"`
    - _Requirements: 2.7, 2.8_

  - [ ]* 1.4 Write property test for email mapping consistency
    - **Property 2: Email mapping consistency** — `mail.google.com`, `com.apple.mail`, `com.microsoft.outlook` MUST all resolve to `"Elegant"`
    - **Validates: Requirements 2.6, 2.7, 2.8**

- [x] 2. Move profile detection to `StartRecording` and add `profile-detected` event
  - [x] 2.1 Move `resolve_system_prompt` call from `StopRecording` to `StartRecording`
    - Inside `DictationEvent::StartRecording`, after capturing `FrontmostApp`, call `resolve_system_prompt` and store result in `DetectedProfile` state
    - Determine `is_auto` flag: `ManualProfileOverride` is `None` AND `auto_detect_profile` setting is not `"false"`
    - Emit `"profile-detected"` event with `{ name, is_auto }` payload
    - _Requirements: 1.1, 1.5, 3.4_

  - [ ]* 2.2 Write property test for detection timing invariant
    - **Property 1: Detection timing invariant** — profile stored in `DetectedProfile` at `StartRecording` MUST be the profile consumed at `StopRecording`
    - **Validates: Requirements 1.1, 9.1**

  - [x] 2.3 Update `StopRecording` to read from `DetectedProfile` cache
    - Replace `resolve_system_prompt` calls in `StopRecording` with a read from `DetectedProfile` state
    - Use `unwrap_or_else(|| resolve_system_prompt(...))` as fallback if state is missing
    - Clear `DetectedProfile` at end of `StopRecording` and `CancelRecording`
    - _Requirements: 1.1, 9.1, 9.2_

  - [ ]* 2.4 Write property test for manual override priority
    - **Property 3: Manual override priority** — when `ManualProfileOverride` is set, `resolve_system_prompt` MUST return that profile regardless of active app
    - **Validates: Requirements 5.1, 4.3_

- [x] 3. Add URL detection timeout and new Tauri commands
  - [x] 3.1 Wrap `get_browser_tab_url` with 50ms timeout using `mpsc` channel
    - In `detect_profile_keyword_for_pid`, spawn a thread to call `get_browser_tab_url`
    - Use `rx.recv_timeout(Duration::from_millis(50))` to enforce the timeout
    - Fall back to bundle-ID matching if timeout elapses
    - _Requirements: 8.2, 8.3, 7.2_

  - [ ]* 3.2 Write property test for URL timeout
    - **Property 6: URL timeout** — `detect_profile_keyword_for_pid` MUST complete within 100ms total
    - **Validates: Requirements 8.1, 8.3**

  - [x] 3.3 Add `set_manual_profile_override` Tauri command in `main.rs`
    - Implement `#[tauri::command] fn set_manual_profile_override(app, profile_name: Option<String>)`
    - When a recording is active (`RecordingState` is recording), also update `DetectedProfile` so the current recording uses the new profile (Req 4.5)
    - Register the command in the Tauri builder's `invoke_handler`
    - _Requirements: 4.3, 4.5, 5.1_

  - [x] 3.4 Expose `get_profiles` as a Tauri command if not already registered
    - Check `main.rs` for an existing `get_profiles` command; add it if missing
    - Return `Vec<db::Profile>` from the DB connection
    - Register in `invoke_handler`
    - _Requirements: 4.2_

- [x] 4. Checkpoint — Ensure all Rust tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 5. Add `ProfileDetectedEvent` and listener to `useTranscription.ts`
  - [x] 5.1 Define `ProfileDetectedEvent` interface and add `activeProfile` state
    - Add `export interface ProfileDetectedEvent { name: string; is_auto: boolean; }` to `useTranscription.ts`
    - Add `const [activeProfile, setActiveProfile] = useState<ProfileDetectedEvent | null>(null)` inside the hook
    - _Requirements: 1.5, 3.1_

  - [x] 5.2 Listen for `"profile-detected"` event and clear on idle
    - In the `useEffect` that sets up Tauri event listeners, add `listen<ProfileDetectedEvent>("profile-detected", (e) => setActiveProfile(e.payload))`
    - When status transitions to `"idle"`, call `setActiveProfile(null)`
    - Return `activeProfile` from the hook
    - _Requirements: 1.5, 3.4_

- [x] 6. Create `useProfiles` hook
  - [x] 6.1 Implement `src/hooks/useProfiles.ts`
    - Fetch profiles on mount via `invoke<Profile[]>("get_profiles")`
    - Listen for `"profiles-updated"` event to refresh the list
    - Return the `profiles` array
    - _Requirements: 4.2_

- [x] 7. Create `ProfilePill` component
  - [x] 7.1 Implement `src/components/ProfilePill.tsx`
    - Accept `profileName: string`, `isAuto: boolean`, `onClick: () => void` props
    - Truncate names longer than 8 characters with "…" (display `name.slice(0,7) + "…"`)
    - Show `auto_awesome` Material Symbol icon when `isAuto` is true
    - Apply pill styling: `bg-white/10 hover:bg-white/20`, `text-[10px] font-bold uppercase tracking-wider`
    - _Requirements: 3.1, 3.2, 3.5_

  - [ ]* 7.2 Write property test for ProfilePill truncation
    - **Property 7: ProfilePill truncation** — names > 8 chars MUST be truncated with "…"; names ≤ 8 chars MUST be shown in full
    - **Validates: Requirements 3.5**

- [x] 8. Create `ProfilePicker` component
  - [x] 8.1 Implement `src/components/ProfilePicker.tsx`
    - Accept `profiles: Profile[]`, `currentProfileName: string`, `onSelect: (name: string | null) => void`, `onClose: () => void` props
    - Render an "Auto" option at the top (calls `onSelect(null)`) with `auto_awesome` icon
    - Render each profile as a button with its icon and name; highlight the currently active profile
    - Apply floating popover styling: `bg-[#0A0A0A]/90 backdrop-blur-[40px] border border-white/10 rounded-[16px]`
    - _Requirements: 4.1, 4.2, 5.4_

  - [x] 8.2 Add click-outside handler to `ProfilePicker`
    - Use a `ref` on the container div and a `mousedown` listener on `document`
    - Call `onClose()` when the click target is outside the ref element
    - _Requirements: 4.6_

  - [ ]* 8.3 Write property test for picker closes on outside click
    - **Property 8: Picker closes on outside click** — clicking outside the popover MUST close it without changing the active profile
    - **Validates: Requirements 4.6**

- [x] 9. Integrate `ProfilePill` and `ProfilePicker` into `RecorderPill.tsx`
  - [x] 9.1 Add `activeProfile` and `profiles` props to `RecorderPill`
    - Extend `RecorderPillProps` with `activeProfile?: ProfileDetectedEvent | null` and `profiles?: Profile[]`
    - Add `pickerOpen` local state: `const [pickerOpen, setPickerOpen] = useState(false)`
    - _Requirements: 3.1, 4.1_

  - [x] 9.2 Render `ProfilePill` inside the recording state section
    - When `isRecording && activeProfile`, render `<ProfilePill profileName={activeProfile.name} isAuto={activeProfile.is_auto} onClick={() => setPickerOpen(true)} />`
    - Position it between the waveform bars and the app icon in the pill layout
    - _Requirements: 3.1, 3.2, 4.1_

  - [x] 9.3 Render `ProfilePicker` popover and wire `set_manual_profile_override`
    - When `pickerOpen`, render `<ProfilePicker>` positioned above the pill (`bottom-[calc(100%+8px)]`)
    - On profile select: call `invoke("set_manual_profile_override", { profileName: name })`, then `setPickerOpen(false)`
    - On close: call `setPickerOpen(false)`
    - _Requirements: 4.3, 4.4, 4.5_

- [x] 10. Update `App.tsx` to pass `activeProfile` and `profiles` to `RecorderPill`
  - [x] 10.1 Consume `useProfiles` and `activeProfile` in `App.tsx`
    - Import and call `useProfiles()` to get the `profiles` array
    - Destructure `activeProfile` from `useTranscription()`
    - Pass both as props to `<RecorderPill>`
    - _Requirements: 3.1, 4.2_

- [-] 11. Final checkpoint — Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for a faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation at the Rust/frontend boundary
- Property tests validate universal correctness properties from the design
- Unit tests validate specific examples and edge cases
- Requirement 6 (Settings toggle for auto-detect) is covered by the existing `auto_detect_profile` DB setting read in task 2.1; a Settings UI toggle is not included here as it requires changes to the Settings panel outside the scope of the files listed in the design
