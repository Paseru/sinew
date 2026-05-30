import { useEffect } from "react";
import { Icon } from "@iconify/react";
import { save } from "@tauri-apps/plugin-dialog";
import { api } from "../lib/ipc";

type Props = {
  x: number;
  y: number;
  /** URL the <img> uses — file:// via convertFileSrc, or data:base64. */
  imageSrc: string;
  /** Absolute path on disk, used for Save As / Reveal / Open. */
  absolutePath: string;
  /** Default filename suggested in the save dialog. */
  fileName: string;
  onClose: () => void;
};

// Estimated menu footprint — used the same way FileTree's TreeContextMenu
// does, to clamp the menu inside the viewport on open.
const MENU_WIDTH = 220;
const MENU_HEIGHT = 196;

/**
 * Custom right-click menu for the image viewer. Replaces the half-broken
 * native WKWebView menu (Download Image / Open in New Window / Share…)
 * with actions that actually work in Tauri. Reuses the `.tree-menu*`
 * design tokens to stay visually consistent with the file tree menu.
 */
export function ImageContextMenu({
  x,
  y,
  imageSrc,
  absolutePath,
  fileName,
  onClose,
}: Props) {
  // Close on outside pointerdown, Escape, scroll, or window resize — same
  // contract as the FileTree menu, so right-click behaves consistently
  // across the app.
  useEffect(() => {
    const onPointerDown = () => onClose();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    const onScroll = () => onClose();
    window.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("resize", onScroll);
    document.addEventListener("scroll", onScroll, true);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("resize", onScroll);
      document.removeEventListener("scroll", onScroll, true);
    };
  }, [onClose]);

  const clampedX =
    typeof window === "undefined"
      ? x
      : Math.max(8, Math.min(x, window.innerWidth - MENU_WIDTH - 8));
  const clampedY =
    typeof window === "undefined"
      ? y
      : Math.max(8, Math.min(y, window.innerHeight - MENU_HEIGHT - 8));

  const runAction = (action: () => Promise<void>) => () => {
    onClose();
    void action().catch((err) => console.error("[image-menu] action failed", err));
  };

  const copyImage = runAction(async () => {
    const response = await fetch(imageSrc);
    const blob = await response.blob();
    // ClipboardItem only reliably accepts image/png across browsers, so
    // when the file isn't already PNG we re-encode it through a canvas.
    const pngBlob = await ensurePngBlob(blob);
    await navigator.clipboard.write([
      new ClipboardItem({ "image/png": pngBlob }),
    ]);
  });

  const saveImageAs = runAction(async () => {
    const ext = extensionFromName(fileName);
    const filters =
      ext.length > 0
        ? [{ name: "Image", extensions: [ext] }]
        : [{ name: "All files", extensions: ["*"] }];
    const dest = await save({ defaultPath: fileName, filters });
    if (!dest) return;
    await api.copyFileToPath(absolutePath, dest);
  });

  const revealInFinder = runAction(() => api.revealAbsolutePath(absolutePath));
  const openWithDefault = runAction(() => api.openPathWithDefaultApp(absolutePath));

  return (
    <div
      className="tree-menu"
      role="menu"
      style={{ left: clampedX, top: clampedY }}
      onPointerDown={(event) => event.stopPropagation()}
      onContextMenu={(event) => event.preventDefault()}
    >
      <MenuItem icon="solar:copy-linear" label="Copy Image" onClick={copyImage} />
      <MenuItem
        icon="solar:download-square-linear"
        label="Save Image As…"
        onClick={saveImageAs}
      />
      <MenuSeparator />
      <MenuItem
        icon="solar:square-top-down-linear"
        label="Open with Default App"
        onClick={openWithDefault}
      />
      <MenuItem
        icon="solar:folder-open-linear"
        label={revealLabel()}
        onClick={revealInFinder}
      />
    </div>
  );
}

function MenuItem({
  icon,
  label,
  onClick,
}: {
  icon: string;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className="tree-menu__item"
      data-danger="false"
      role="menuitem"
      onClick={onClick}
    >
      <Icon icon={icon} width={14} height={14} />
      <span>{label}</span>
    </button>
  );
}

function MenuSeparator() {
  return <div className="tree-menu__separator" role="separator" />;
}

function extensionFromName(name: string): string {
  const dot = name.lastIndexOf(".");
  if (dot < 0 || dot === name.length - 1) return "";
  return name.slice(dot + 1).toLowerCase();
}

function revealLabel(): string {
  const platform =
    typeof navigator !== "undefined" ? navigator.platform.toLowerCase() : "";
  if (platform.includes("mac")) return "Reveal in Finder";
  if (platform.includes("win")) return "Show in Explorer";
  return "Show in File Manager";
}

async function ensurePngBlob(blob: Blob): Promise<Blob> {
  if (blob.type === "image/png") return blob;
  const bitmap = await createImageBitmap(blob);
  const canvas = document.createElement("canvas");
  canvas.width = bitmap.width;
  canvas.height = bitmap.height;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("canvas 2d context unavailable");
  ctx.drawImage(bitmap, 0, 0);
  bitmap.close?.();
  return new Promise<Blob>((resolve, reject) => {
    canvas.toBlob((out) => {
      if (!out) reject(new Error("canvas toBlob failed"));
      else resolve(out);
    }, "image/png");
  });
}
