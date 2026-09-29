/** Minimal line icons, 1.5px stroke on a 16px grid, drawn in currentColor. */
import type { SVGProps } from "react";

function Icon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      {...props}
    />
  );
}

export const MinimizeIcon = () => (
  <Icon>
    <path d="M4 8h8" />
  </Icon>
);

export const MaximizeIcon = () => (
  <Icon>
    <rect x="4" y="4" width="8" height="8" rx="1.5" />
  </Icon>
);

export const CloseIcon = () => (
  <Icon>
    <path d="M4.5 4.5l7 7M11.5 4.5l-7 7" />
  </Icon>
);

export const ArrowUpIcon = () => (
  <Icon>
    <path d="M8 12.5V3.5M4 7.5l4-4 4 4" />
  </Icon>
);

export const MicIcon = () => (
  <Icon>
    <rect x="6" y="2" width="4" height="8" rx="2" />
    <path d="M3.5 7.5a4.5 4.5 0 009 0M8 12v2" />
  </Icon>
);

export const StopIcon = () => (
  <Icon>
    <rect x="4.5" y="4.5" width="7" height="7" rx="1.5" />
  </Icon>
);

/** The SERSHI mark, simplified for small sizes. */
export const Mark = ({ size = 18 }: { size?: number }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true" focusable="false">
    <ellipse
      cx="12"
      cy="12"
      rx="10"
      ry="4.2"
      fill="none"
      stroke="currentColor"
      strokeOpacity="0.55"
      strokeWidth="1.3"
      transform="rotate(-28 12 12)"
    />
    <circle cx="12" cy="12" r="3.4" fill="currentColor" />
    <circle cx="20.2" cy="7.6" r="1.1" fill="currentColor" />
  </svg>
);

export const HomeIcon = () => (
  <Icon>
    <path d="M2.75 7.25 8 2.75l5.25 4.5V13a.75.75 0 0 1-.75.75H3.5a.75.75 0 0 1-.75-.75Z" />
    <path d="M6.25 13.75v-4h3.5v4" />
  </Icon>
);

export const ActivityIcon = () => (
  <Icon>
    <path d="M1.75 8h2.5l2-4.5 3.5 9 2-4.5h2.5" />
  </Icon>
);

export const SettingsIcon = () => (
  <Icon>
    <path d="M2.5 4.5h7M12.5 4.5h1M2.5 11.5h1M6.5 11.5h7" />
    <circle cx="11" cy="4.5" r="1.5" />
    <circle cx="5" cy="11.5" r="1.5" />
  </Icon>
);

export const RefreshIcon = () => (
  <Icon>
    <path d="M13 8a5 5 0 1 1-1.46-3.54M13 2.75v2.5h-2.5" />
  </Icon>
);

/** The trusted-approval mark: a shield around SERSHI's core. */
export const ApprovalMark = () => (
  <Icon>
    <path d="M8 1.75 13.25 3.5v4c0 3.1-2.2 5.5-5.25 6.75C4.95 13 2.75 10.6 2.75 7.5v-4Z" />
    <circle cx="8" cy="7.5" r="1.75" fill="currentColor" stroke="none" />
  </Icon>
);
