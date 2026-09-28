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
