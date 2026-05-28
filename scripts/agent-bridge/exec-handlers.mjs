import fs from "node:fs";
import path from "node:path";
import { create } from "@bufbuild/protobuf";
import {
  DeleteRejectedSchema,
  DeleteResultSchema,
  DeleteSuccessSchema,
  LsDirectoryTreeNodeSchema,
  LsRejectedSchema,
  LsResultSchema,
  LsSuccessSchema,
  ReadErrorSchema,
  ReadResultSchema,
  ReadSuccessSchema,
  WriteRejectedSchema,
  WriteResultSchema,
  WriteSuccessSchema,
} from "./vendor/agent_pb.ts";

const READ_LIMIT = 512 * 1024;

export function resolveWorkspacePath(workspaceRoot, rawPath) {
  const root = workspaceRoot?.trim() || process.cwd();
  const target = path.resolve(root, rawPath || ".");
  const normalizedRoot = path.resolve(root);
  if (!target.startsWith(normalizedRoot)) {
    throw new Error("path outside workspace");
  }
  return target;
}

function shallowLayout(workspaceRoot, maxEntries = 80) {
  const root = workspaceRoot?.trim() || process.cwd();
  const childrenDirs = [];
  const childrenFiles = [];
  try {
    const entries = fs.readdirSync(root, { withFileTypes: true });
    for (const entry of entries.slice(0, maxEntries)) {
      const absPath = path.join(root, entry.name);
      if (entry.isDirectory()) {
        childrenDirs.push(
          create(LsDirectoryTreeNodeSchema, {
            absPath,
            childrenDirs: [],
            childrenFiles: [],
            childrenWereProcessed: false,
            fullSubtreeExtensionCounts: {},
            numFiles: 0,
          }),
        );
      } else if (entry.isFile()) {
        childrenFiles.push({ name: entry.name });
      }
    }
  } catch {
    /* ignore */
  }
  return create(LsDirectoryTreeNodeSchema, {
    absPath: root,
    childrenDirs,
    childrenFiles,
    childrenWereProcessed: true,
    fullSubtreeExtensionCounts: {},
    numFiles: childrenFiles.length,
  });
}

export function buildProjectLayout(workspaceRoot, snapshot) {
  if (snapshot?.projectLayout) {
    return undefined;
  }
  return shallowLayout(workspaceRoot);
}

export function handleReadArgs(execMsg, args, workspaceRoot, sendExecResult) {
  const filePath = args.path || args.filePath || "";
  try {
    const full = resolveWorkspacePath(workspaceRoot, filePath);
    const buf = fs.readFileSync(full);
    const truncated = buf.length > READ_LIMIT;
    const slice = truncated ? buf.subarray(0, READ_LIMIT) : buf;
    const content = slice.toString("utf8");
    const totalLines = content.split("\n").length;
    sendExecResult(
      execMsg,
      "readResult",
      create(ReadResultSchema, {
        result: {
          case: "success",
          value: create(ReadSuccessSchema, {
            path: filePath,
            totalLines,
            fileSize: BigInt(buf.length),
            truncated,
            output: { case: "content", value: content },
          }),
        },
      }),
    );
  } catch (err) {
    sendExecResult(
      execMsg,
      "readResult",
      create(ReadResultSchema, {
        result: {
          case: "error",
          value: create(ReadErrorSchema, {
            path: filePath,
            error: String(err?.message || err),
          }),
        },
      }),
    );
  }
}

export function handleLsArgs(execMsg, args, workspaceRoot, sendExecResult) {
  const dirPath = args.path || args.targetDirectory || ".";
  try {
    const full = resolveWorkspacePath(workspaceRoot, dirPath);
    const tree = shallowLayout(full, 120);
    sendExecResult(
      execMsg,
      "lsResult",
      create(LsResultSchema, {
        result: {
          case: "success",
          value: create(LsSuccessSchema, { directoryTreeRoot: tree }),
        },
      }),
    );
  } catch (err) {
    sendExecResult(
      execMsg,
      "lsResult",
      create(LsResultSchema, {
        result: {
          case: "rejected",
          value: create(LsRejectedSchema, {
            path: dirPath,
            reason: String(err?.message || err),
          }),
        },
      }),
    );
  }
}

export function handleWriteArgs(execMsg, args, workspaceRoot, sendExecResult) {
  const filePath = args.path || args.filePath || args.target_file || "";
  const oldString = args.old_string ?? args.oldString ?? "";
  const newString =
    args.new_string ??
    args.newString ??
    args.contents ??
    args.content ??
    args.text ??
    args.replacement ??
    "";
  try {
    const full = resolveWorkspacePath(workspaceRoot, filePath);
    fs.mkdirSync(path.dirname(full), { recursive: true });
    let content = String(newString);
    if (oldString && fs.existsSync(full)) {
      const prior = fs.readFileSync(full, "utf8");
      if (!prior.includes(oldString)) {
        throw new Error("old_string not found in file");
      }
      content = prior.replace(oldString, newString);
    } else if (!oldString) {
      content = String(newString);
    }
    fs.writeFileSync(full, content, "utf8");
    const lines = String(content).split("\n").length;
    sendExecResult(
      execMsg,
      "writeResult",
      create(WriteResultSchema, {
        result: {
          case: "success",
          value: create(WriteSuccessSchema, {
            path: filePath,
            linesCreated: lines,
            fileSize: Buffer.byteLength(String(content), "utf8"),
          }),
        },
      }),
    );
  } catch (err) {
    sendExecResult(
      execMsg,
      "writeResult",
      create(WriteResultSchema, {
        result: {
          case: "rejected",
          value: create(WriteRejectedSchema, {
            path: filePath,
            reason: String(err?.message || err),
          }),
        },
      }),
    );
  }
}

export function handleDeleteArgs(execMsg, args, workspaceRoot, sendExecResult) {
  const filePath = args.path || args.filePath || "";
  try {
    const full = resolveWorkspacePath(workspaceRoot, filePath);
    const prev = fs.existsSync(full) ? fs.readFileSync(full, "utf8") : "";
    const size = BigInt(fs.statSync(full).size);
    fs.unlinkSync(full);
    sendExecResult(
      execMsg,
      "deleteResult",
      create(DeleteResultSchema, {
        result: {
          case: "success",
          value: create(DeleteSuccessSchema, {
            path: filePath,
            deletedFile: filePath,
            fileSize: size,
            prevContent: prev,
          }),
        },
      }),
    );
  } catch (err) {
    sendExecResult(
      execMsg,
      "deleteResult",
      create(DeleteResultSchema, {
        result: {
          case: "rejected",
          value: create(DeleteRejectedSchema, {
            path: filePath,
            reason: String(err?.message || err),
          }),
        },
      }),
    );
  }
}
