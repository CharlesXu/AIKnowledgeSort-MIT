export type IconName =
  | "archive"
  | "chevron"
  | "document"
  | "folder"
  | "graph"
  | "inbox"
  | "layers"
  | "search"
  | "settings";

interface IconProps {
  readonly name: IconName;
  readonly size?: number;
}

const paths: Record<IconName, React.ReactNode> = {
  archive: (
    <>
      <path d="M4 7.5h16v11H4z" />
      <path d="M3 4.5h18v3H3zm6 7h6" />
    </>
  ),
  chevron: <path d="m9 6 6 6-6 6" />,
  document: (
    <>
      <path d="M6 3h8l4 4v14H6z" />
      <path d="M14 3v5h4M9 12h6m-6 4h6" />
    </>
  ),
  folder: <path d="M3 6.5h7l2-2h9v15H3z" />,
  graph: (
    <>
      <circle cx="6" cy="7" r="2" />
      <circle cx="18" cy="6" r="2" />
      <circle cx="13" cy="18" r="2" />
      <path d="m8 7 8-1m1 2-3 8M7 9l5 7" />
    </>
  ),
  inbox: (
    <>
      <path d="M4 4h16v16H4z" />
      <path d="M4 14h5l2 2h2l2-2h5" />
    </>
  ),
  layers: (
    <>
      <path d="m12 3 9 5-9 5-9-5z" />
      <path d="m4 13 8 4 8-4m-16 5 8 4 8-4" />
    </>
  ),
  search: (
    <>
      <circle cx="10.5" cy="10.5" r="6.5" />
      <path d="m15.5 15.5 5 5" />
    </>
  ),
  settings: (
    <>
      <circle cx="12" cy="12" r="3" />
      <path d="M12 2v3m0 14v3M2 12h3m14 0h3M5 5l2 2m10 10 2 2M19 5l-2 2M7 17l-2 2" />
    </>
  ),
};

export function Icon({ name, size = 16 }: IconProps) {
  return (
    <svg
      aria-hidden="true"
      className="icon"
      fill="none"
      height={size}
      viewBox="0 0 24 24"
      width={size}
    >
      <g
        stroke="currentColor"
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="1.6"
      >
        {paths[name]}
      </g>
    </svg>
  );
}
