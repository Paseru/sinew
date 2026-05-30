const fs = require('fs');
let code = fs.readFileSync('src/components/chat/TodoStrip.tsx', 'utf8');

// The replacement logic for Kanban rendering
const replacement = \      {active !== "messages" && visibleTaskEntries.length > 0 && (
        active === "team" ? (
          <div className="todo-strip__kanban" style={{
            display: "flex", gap: "8px", padding: "8px 12px", height: "100%", overflowX: "auto", overflowY: "hidden", alignItems: "flex-start"
          }}>
            {["pending", "in_progress", "blocked", "completed"].map((status) => {
              const columnTasks = visibleTaskEntries.filter((e) => e.task.status === status);
              if (columnTasks.length === 0 && status !== "in_progress") return null;
              
              const statusColors = {
                pending: "var(--text-3)",
                in_progress: "var(--accent-hi)",
                blocked: "var(--danger)",
                completed: "var(--ok)"
              };
              const statusLabels = {
                pending: "À faire",
                in_progress: "En cours",
                blocked: "Bloqué",
                completed: "Terminé"
              };
              
              return (
                <div key={status} style={{
                  display: "flex", flexDirection: "column", gap: "6px", flex: "1 1 220px", minWidth: "200px", maxWidth: "300px",
                  background: "rgba(255, 255, 255, 0.03)", borderRadius: "var(--r-card)", padding: "8px", height: "100%", maxHeight: "100%", overflowY: "auto", border: "1px solid var(--line-1)"
                }}>
                  <div style={{ display: "flex", alignItems: "center", gap: "6px", fontSize: "11px", fontWeight: 600, color: statusColors[status], paddingBottom: "4px", borderBottom: "1px solid var(--line-1)", textTransform: "uppercase", letterSpacing: "0.5px" }}>
                    <TaskStatusMark status={status as any} />
                    {statusLabels[status]} ({columnTasks.length})
                  </div>
                  <div style={{ display: "flex", flexDirection: "column", gap: "6px" }}>
                    {columnTasks.map(({ task, index }) => (
                      <div key={task.id} style={{
                        display: "flex", flexDirection: "column", gap: "4px", padding: "8px", background: "var(--bg-2)", borderRadius: "var(--r-med)", border: "1px solid var(--line-1)", fontSize: "12px", color: status === "completed" ? "var(--text-3)" : "var(--text-1)", opacity: status === "completed" ? 0.7 : 1
                      }}>
                        <div style={{ display: "flex", alignItems: "flex-start", gap: "6px" }}>
                          <span style={{ flex: 1, wordBreak: "break-word", lineHeight: 1.4 }}>{task.text}</span>
                        </div>
                        <TeamTaskMeta task={task as any} index={index} agentColors={teamAgentColors} />
                      </div>
                    ))}
                  </div>
                </div>
              );
            })}
          </div>
        ) : (
          <div className="todo-strip__list">
            {visibleTaskEntries.map(({ task, index }) => (
              <div
                key={task.id}
                className="todo-strip__item"
                data-panel={active}
                data-status={task.status}
              >
                <TaskStatusMark status={task.status} />
                <span className="todo-strip__text">
                  {task.text}
                  {active === "team" && (
                    <TeamTaskMeta
                      task={task as any}
                      index={index}
                      agentColors={teamAgentColors}
                    />
                  )}
                </span>
              </div>
            ))}
          </div>
        )
      )}\;

code = code.replace(
  /\{\s*active !== "messages" && visibleTaskEntries\.length > 0 && \(\s*<div className="todo-strip__list">[\s\S]*?<\/div>\s*\)\s*\}/,
  replacement
);

fs.writeFileSync('src/components/chat/TodoStrip.tsx', code);
