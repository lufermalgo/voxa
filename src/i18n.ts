export const translations = {
  en: {
    // Recorder Pill
    processing: "Processing...",
    recording: "Recording",
    sent: "Sent",
    loading: "Loading...",
    provisioning: "Provisioning AI (Metal)...",
    downloading_model: "Downloading {model} ({progress}%)",
    ready: "Ready",
    error_logs: "Error. Check logs.",
    
    // Settings Panel Header
    app_subtitle: "Intelligent Voice Refinement",
    navigation: "Navigation",
    
    // Tabs
    history: "History",
    profiles: "Profiles",
    dictionary: "Dictionary",
    general: "General",
    
    // History Section
    voice_history: "Voice History",
    history_subtitle: "All your recent local transcriptions.",
    clear_history: "Clear History",
    confirm_clear: "Are you sure you want to clear all history?",
    no_transcripts: "No transcriptions yet",
    copy_text: "Copy text",
    delete: "Delete",
    
    // Profiles Section
    transformation_profiles: "Transformation Profiles",
    profiles_subtitle: "Define how the AI should refine and format your speech.",
    active: "ACTIVE",
    exact_transcription: "Exact transcription without refinement.",
    edit: "Edit",
    save_profile: "Save Profile",
    name_label: "Profile Name",
    icon_label: "Visual Icon",
    prompt_label: "System Instructions (Prompt)",
    prompt_placeholder: "Describe how you want Voxa to transform what you say...",
    cancel: "Cancel",
    borrar: "Delete",
    confirm_delete_profile: "Are you sure you want to delete {name}?",
    create_new_profile: "Create New Custom Profile",
    new_profile_title: "New Custom Profile",
    writer_example: "e.g., Creative Writer",
    instructions_custom: "Custom Instructions",
    expert_example: "e.g., Act as a legal expert...",
    discard: "Discard",
    create_profile: "Create Profile",
    
    // Dictionary Section
    personal_dictionary: "Personal Dictionary",
    dictionary_subtitle: "Teach Voxa technical terms, brands, or names you use often.",
    dictionary_empty: "Your internal vocabulary is empty.",
    dictionary_placeholder: "e.g., Next.js, Rioplatense, Lufer...",
    add: "ADD",
    
    // General Section
    system_settings: "System Settings",
    settings_subtitle: "Fundamental audio and quick access adjustments.",
    audio_source: "Audio Source (Microphone)",
    auto_detect: "Auto-detect System",
    global_shortcut: "Global Shortcut (Activation)",
    selected_shortcut: "Selected Combination",
    listening: "Listening...",
    click_to_change: "Click to Change",
    transcription_input_lang: "Transcription Speech Language",
    spanish: "SPANISH",
    english: "ENGLISH",
    tip_text: "Tip: Use Cmd+L to start a new instant transcription.",
    
    // Models Section
    models: "Models",
    ai_models: "AI Models",
    models_subtitle: "Local intelligence engines powering Voxa.",
    downloaded: "Downloaded & Ready",
    missing: "Missing - Tap to Download",
    open_folder: "Open Models Folder",
    redownload: "Force Re-download",
    size_mb: "MB",
    path: "Path",
    
    // Footer
    footer_engine: "Voxa v{version}"
  },
  es: {
    // Recorder Pill
    processing: "Procesando...",
    recording: "Grabando",
    sent: "Enviado",
    loading: "Cargando...",
    provisioning: "Proveyendo IA (Metal)...",
    downloading_model: "Descargando {model} ({progress}%)",
    ready: "Listo",
    error_logs: "Error. Revisar logs.",
    
    // Settings Panel Header
    app_subtitle: "Refinamiento de Voz Inteligente",
    navigation: "Navegación",
    
    // Tabs
    history: "Historial",
    profiles: "Perfiles",
    dictionary: "Diccionario",
    general: "General",
    
    // History Section
    voice_history: "Historial de Voz",
    history_subtitle: "Todas tus transcripciones recientes guardadas localmente.",
    clear_history: "Limpiar Historial",
    confirm_clear: "¿Estás seguro de borrar todo el historial?",
    no_transcripts: "No hay transcripciones aún",
    copy_text: "Copiar texto",
    delete: "Eliminar",
    
    // Profiles Section
    transformation_profiles: "Perfiles de Transformación",
    profiles_subtitle: "Definí cómo la IA debe refinar y dar formato a lo que decís.",
    active: "ACTIVO",
    exact_transcription: "Transcripción exacta sin refinamiento.",
    edit: "Editar",
    save_profile: "Guardar Perfil",
    name_label: "Nombre del Perfil",
    icon_label: "Icono Visual",
    prompt_label: "Instrucciones del Sistema (Prompt)",
    prompt_placeholder: "Describí cómo querés que Voxa transforme lo que decís...",
    cancel: "Cancelar",
    borrar: "Borrar",
    confirm_delete_profile: "¿Seguro que querés borrar \"{name}\"?",
    create_new_profile: "Crear Nuevo Perfil Custom",
    new_profile_title: "Nuevo Perfil Personalizado",
    writer_example: "Ej: Escritor Creativo",
    instructions_custom: "Instrucciones Custom",
    expert_example: "Ej: Actuá como un experto legal...",
    discard: "Descartar",
    create_profile: "Crear Perfil",
    
    // Dictionary Section
    personal_dictionary: "Diccionario Personal",
    dictionary_subtitle: "Enseñale a Voxa términos técnicos, marcas o nombres propios que usás seguido.",
    dictionary_empty: "Tu vocabulario interno está vacío.",
    dictionary_placeholder: "Ej: Next.js, Rioplatense, Lufer...",
    add: "AGREGAR",
    
    // General Section
    system_settings: "Configuración del Sistema",
    settings_subtitle: "Ajustes fundamentales de audio y acceso rápido.",
    audio_source: "Fuente de Audio (Micrófono)",
    auto_detect: "Auto-detectar Sistema",
    global_shortcut: "Atajo Global (Activación)",
    selected_shortcut: "Combinación Seleccionada",
    listening: "Escuchando...",
    click_to_change: "Hacer Click para Cambiar",
    transcription_input_lang: "Idioma de Entrada (Voz)",
    spanish: "ESPAÑOL",
    english: "INGLÉS",
    tip_text: "Tip: Usá Cmd+L para iniciar una nueva transcripción instantánea.",
    
    // Models Section
    models: "Modelos",
    ai_models: "Modelos de IA",
    models_subtitle: "Motores de inteligencia local que le dan vida a Voxa.",
    downloaded: "Descargado y Listo",
    missing: "Faltante - Click para Bajar",
    open_folder: "Abrir Carpeta de Modelos",
    redownload: "Forzar Re-descarga",
    size_mb: "MB",
    path: "Ruta",
    
    // Footer
    footer_engine: "Voxa v{version}"
  }
};

export type Locale = "en" | "es";
export type TranslationKeys = keyof typeof translations.en;
