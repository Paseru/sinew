import { useEffect, useState } from "react";
import { DotmSquare5 } from "./DotmSquare5";

function readCompactReasoning(): "disabled" | "compact" | "very-compact" {
  try {
    const val = localStorage.getItem("sinew.compact-reasoning");
    if (val === "very-compact") return "very-compact";
    if (val === "compact" || val === "true") return "compact";
    return "disabled";
  } catch {
    return "disabled";
  }
}

export function PlanningNextMoveBlock() {
  const [compactMode, setCompactMode] = useState<"disabled" | "compact" | "very-compact">(() => {
    return readCompactReasoning();
  });

  useEffect(() => {
    const handler = (event: Event) => {
      const mode = (event as CustomEvent<"disabled" | "compact" | "very-compact">).detail;
      setCompactMode(mode);
    };
    window.addEventListener("sinew:compact-reasoning-changed", handler as any);
    return () => window.removeEventListener("sinew:compact-reasoning-changed", handler as any);
  }, []);

  if (compactMode === "very-compact") {
    return (
      <div className="thinking-block planning-next-move" role="status" style={{ paddingLeft: "10px", paddingTop: "4px" }}>
        <DotmSquare5
          speed={1}
          animated
          className="thinking-block__matrix planning-next-move__matrix"
        />
      </div>
    );
  }

  return (
    <div className="thinking-block planning-next-move" role="status">
      <div
        className="thinking-block__head planning-next-move__head"
        data-streaming="true"
        data-has-content="false"
      >
        <DotmSquare5
          speed={1}
          animated
          className="thinking-block__matrix planning-next-move__matrix"
        />
        <span className="thinking-block__label" data-streaming="true">
          Planning next moves
        </span>
      </div>
    </div>
  );
}
