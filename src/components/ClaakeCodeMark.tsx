import logoUrl from "../assets/claakecode-logo.png";

type Props = {
  size?: number;
  className?: string;
};

/**
 * Claake Code brand mark — transparent logo PNG.
 * Used in the titlebar, Welcome screen, and Settings → About.
 */
export function ClaakeCodeMark({ size = 22, className }: Props) {
  return (
    <img
      src={logoUrl}
      width={size}
      height={size}
      className={className}
      alt="Claake Code"
      draggable={false}
      style={{ display: "block", objectFit: "contain" }}
    />
  );
}
