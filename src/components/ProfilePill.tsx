interface ProfilePillProps {
  profileName: string;
  isAuto: boolean;
  onClick: () => void;
}

export function ProfilePill({ profileName, isAuto, onClick }: ProfilePillProps) {
  const displayName = profileName.length > 8
    ? profileName.slice(0, 7) + "…"
    : profileName;

  return (
    <button
      onClick={onClick}
      className="flex items-center gap-1 px-2 py-0.5 rounded-full bg-white/10 hover:bg-white/20 transition-colors text-white/80 hover:text-white text-[10px] font-bold uppercase tracking-wider flex-shrink-0"
    >
      {isAuto && (
        <span className="material-symbols-outlined !text-[10px] text-primary/80">auto_awesome</span>
      )}
      <span>{displayName}</span>
    </button>
  );
}
