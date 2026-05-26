import {
  Children,
  cloneElement,
  isValidElement,
  memo,
  useState,
  useEffect,
  type ReactElement,
  type ReactNode,
} from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import { Icon } from "@iconify/react";
import { api } from "../../lib/ipc";
import { MermaidDiagram } from "./MermaidDiagram";

type Props = {
  text: string;
  onOpenFile: (path: string) => void;
  workspacePath?: string;
};

type LinkifyOptions = {
  onOpenFile: (path: string) => void;
  workspacePath?: string;
};

const FILE_TOKEN =
  /((?:\.{1,2}\/)?(?:[A-Za-z0-9_.+-]+\/)+[A-Za-z0-9_.+-]+\.[A-Za-z0-9]+(?::\d+(?::\d+)?)?|[A-Za-z0-9_.+-]+\.(?:tsx?|jsx?|rs|toml|json|md|css|scss|html|ya?ml|lock|sh|zsh|bash|py|go|java|kt|swift|sql|env|mjs|cjs|config)(?::\d+(?::\d+)?)?)/g;

function isMermaidLanguage(className?: string): boolean {
  if (!className) return false;
  return className
    .split(/\s+/)
    .some((name) => name === "mermaid" || name === "language-mermaid");
}

function isFileToken(value: string): boolean {
  FILE_TOKEN.lastIndex = 0;
  const match = FILE_TOKEN.exec(value.trim());
  return match?.[0] === value.trim();
}

function FileLink({
  path,
  children,
  variant,
  onOpenFile,
  workspacePath,
}: {
  path: string;
  children: ReactNode;
  variant?: "code";
  onOpenFile: (path: string) => void;
  workspacePath?: string;
}) {
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);

  useEffect(() => {
    if (!menu) return;
    const handleClose = () => setMenu(null);
    window.addEventListener("pointerdown", handleClose);
    return () => window.removeEventListener("pointerdown", handleClose);
  }, [menu]);

  const handleReveal = async () => {
    if (!workspacePath) return;
    try {
      const resolved = await api.resolveTerminalPath(workspacePath, path);
      await api.revealAbsolutePath(resolved.absolutePath);
    } catch (err) {
      console.error(err);
    }
  };

  const handleExecute = async () => {
    if (!workspacePath) return;
    try {
      const resolved = await api.resolveTerminalPath(workspacePath, path);
      await api.openPathWithDefaultApp(resolved.absolutePath);
    } catch (err) {
      console.error(err);
    }
  };

  return (
    <>
      <button
        type="button"
        className="chat-file-link"
        data-variant={variant}
        title="Left-click to open. Right-click for options."
        onClick={(event) => {
          event.stopPropagation();
          onOpenFile(path);
        }}
        onContextMenu={(event) => {
          event.preventDefault();
          event.stopPropagation();
          setMenu({ x: event.clientX, y: event.clientY });
        }}
      >
        {children}
      </button>
      {menu && (
        <div
          className="tree-menu"
          role="menu"
          style={{
            position: "fixed",
            left: menu.x,
            top: menu.y,
            zIndex: 9999,
          }}
          onPointerDown={(event) => event.stopPropagation()}
          onContextMenu={(event) => event.preventDefault()}
        >
          <button
            type="button"
            className="tree-menu__item"
            role="menuitem"
            onClick={() => {
              setMenu(null);
              onOpenFile(path);
            }}
          >
            <Icon icon="solar:file-text-linear" width={14} height={14} />
            <span>Open in Editor</span>
          </button>

          {workspacePath && (
            <>
              <button
                type="button"
                className="tree-menu__item"
                role="menuitem"
                onClick={() => {
                  setMenu(null);
                  void handleReveal();
                }}
              >
                <Icon icon="solar:folder-open-linear" width={14} height={14} />
                <span>Reveal in Explorer</span>
              </button>
              <button
                type="button"
                className="tree-menu__item"
                role="menuitem"
                onClick={() => {
                  setMenu(null);
                  void handleExecute();
                }}
              >
                <Icon icon="solar:play-linear" width={14} height={14} />
                <span>Execute / Run</span>
              </button>
            </>
          )}
        </div>
      )}
    </>
  );
}

function linkifyText(text: string, { onOpenFile, workspacePath }: LinkifyOptions): ReactNode[] {
  const nodes: ReactNode[] = [];
  let lastIndex = 0;

  FILE_TOKEN.lastIndex = 0;
  for (const match of text.matchAll(FILE_TOKEN)) {
    const index = match.index ?? 0;
    const value = match[0];
    if (index > lastIndex) {
      nodes.push(text.slice(lastIndex, index));
    }
    nodes.push(
      <FileLink key={`${value}-${index}`} path={value} onOpenFile={onOpenFile} workspacePath={workspacePath}>
        {value}
      </FileLink>,
    );
    lastIndex = index + value.length;
  }

  if (lastIndex < text.length) {
    nodes.push(text.slice(lastIndex));
  }

  return nodes;
}

function childrenToString(children: ReactNode): string {
  return Children.toArray(children)
    .map((child) => {
      if (typeof child === "string") return child;
      if (typeof child === "number") return String(child);
      if (!isValidElement(child)) return "";
      const props = child.props as { children?: ReactNode };
      return childrenToString(props.children);
    })
    .join("");
}

function linkifyChildren(children: ReactNode, options: LinkifyOptions): ReactNode {
  return Children.map(children, (child) => {
    if (typeof child === "string") return linkifyText(child, options);
    if (!isValidElement(child)) return child;
    if (child.type === "a" || child.type === "code" || child.type === "pre") {
      return child;
    }

    const props = child.props as { children?: ReactNode };
    if (props.children === undefined) return child;

    return cloneElement(
      child as ReactElement<{ children?: ReactNode }>,
      undefined,
      linkifyChildren(props.children, options),
    );
  });
}

export function FileLinkedText({ text, onOpenFile, workspacePath }: Props) {
  return <>{linkifyText(text, { onOpenFile, workspacePath })}</>;
}

/**
 * Markdown renderer tuned for chat. GFM + highlight.js. Memoized so
 * streaming token-by-token re-renders don't trash the whole tree.
 */
export const Markdown = memo(function Markdown({ text, onOpenFile, workspacePath }: Props) {
  return (
    <div className="md">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[[rehypeHighlight, { detect: true, ignoreMissing: true }]]}
        components={{
          pre({ children }) {
            // `code` already swaps fenced ```mermaid blocks for an
            // interactive <MermaidDiagram />. Unwrap the surrounding
            // <pre> in that case so the diagram isn't boxed by code
            // styling; otherwise render a plain <pre>.
            const only =
              Children.count(children) === 1
                ? Children.toArray(children)[0]
                : null;
            if (isValidElement(only) && only.type === MermaidDiagram) {
              return only;
            }
            if (isValidElement<{ className?: string; children?: ReactNode }>(only)) {
              if (isMermaidLanguage(only.props.className)) {
                return (
                  <MermaidDiagram source={childrenToString(only.props.children)} />
                );
              }
            }
            return <pre>{children}</pre>;
          },
          p({ children }) {
            return <p>{linkifyChildren(children, { onOpenFile, workspacePath })}</p>;
          },
          li({ children }) {
            return <li>{linkifyChildren(children, { onOpenFile, workspacePath })}</li>;
          },
          h1({ children }) {
            return <h1>{linkifyChildren(children, { onOpenFile, workspacePath })}</h1>;
          },
          h2({ children }) {
            return <h2>{linkifyChildren(children, { onOpenFile, workspacePath })}</h2>;
          },
          h3({ children }) {
            return <h3>{linkifyChildren(children, { onOpenFile, workspacePath })}</h3>;
          },
          h4({ children }) {
            return <h4>{linkifyChildren(children, { onOpenFile, workspacePath })}</h4>;
          },
          code({ children, className }) {
            const value = childrenToString(children);
            if (isMermaidLanguage(className)) {
              return <MermaidDiagram source={value} />;
            }
            if (!className && isFileToken(value)) {
              return (
                <FileLink
                  path={value}
                  variant="code"
                  onOpenFile={onOpenFile}
                  workspacePath={workspacePath}
                >
                  {value}
                </FileLink>
              );
            }
            return <code className={className}>{children}</code>;
          },
          table({ children }) {
            // Wrap the table so horizontal scrolling lives on a block-level
            // ancestor. The <table> itself stays a real display: table so
            // the auto column-width algorithm distributes space correctly
            // (otherwise display: block on the table collapses left columns).
            return (
              <div className="md-table-wrap">
                <table>{children}</table>
              </div>
            );
          },
          a({ href, children }) {
            const value = href ?? childrenToString(children);
            if (isFileToken(value)) {
              return (
                <FileLink path={value} onOpenFile={onOpenFile} workspacePath={workspacePath}>
                  {children}
                </FileLink>
              );
            }
            return (
              <a
                href={href}
                target="_blank"
                rel="noreferrer"
                onClick={(event) => {
                  if (!href) return;
                  event.preventDefault();
                  event.stopPropagation();
                  void api.openExternalUrl(href);
                }}
              >
                {children}
              </a>
            );
          },
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
});
