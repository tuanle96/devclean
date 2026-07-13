import { spawn } from "node:child_process";
import * as os from "node:os";
import * as path from "node:path";
import * as vscode from "vscode";
import { cleanArguments, Filters, reportArguments, scanArguments } from "./commands";

interface ScanReport {
  candidates: Array<{ path: string; bytes: number }>;
  total_bytes: number;
}

let status: vscode.StatusBarItem;
let output: vscode.OutputChannel;

export function activate(context: vscode.ExtensionContext): void {
  output = vscode.window.createOutputChannel("DevCleaner");
  status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 50);
  status.command = "devclean.scan";
  status.text = "$(search) DevCleaner";
  status.tooltip = "Scan this workspace for rebuildable artifacts";
  status.show();

  context.subscriptions.push(
    output,
    status,
    vscode.commands.registerCommand("devclean.scan", scanWorkspace),
    vscode.commands.registerCommand("devclean.openReport", openReport),
    vscode.commands.registerCommand("devclean.dryRun", previewCleanup),
    vscode.commands.registerCommand("devclean.clean", cleanAfterConfirmation),
  );
  void scanWorkspace(false);
}

export function deactivate(): void {}

async function scanWorkspace(showOutput = true): Promise<void> {
  const root = workspaceRoot();
  if (!root) return;
  status.text = "$(sync~spin) DevCleaner";
  try {
    const result = await run(scanArguments(root, filters()));
    const report = JSON.parse(result.stdout) as ScanReport;
    status.text = `$(database) ${formatBytes(report.total_bytes)} reclaimable`;
    status.tooltip = `${report.candidates.length} rebuildable candidates. Click to rescan.`;
    if (showOutput) {
      output.appendLine(result.stdout);
      output.show(true);
    }
  } catch (error) {
    status.text = "$(warning) DevCleaner";
    status.tooltip = String(error);
    if (showOutput) void vscode.window.showErrorMessage(`DevCleaner scan failed: ${String(error)}`);
  }
}

async function openReport(): Promise<void> {
  const root = workspaceRoot();
  if (!root) return;
  const destination = path.join(os.tmpdir(), `devclean-vscode-${Date.now()}.html`);
  await run(reportArguments(root, destination, filters()));
  await vscode.env.openExternal(vscode.Uri.file(destination));
}

async function previewCleanup(): Promise<void> {
  const root = workspaceRoot();
  if (!root) return;
  const result = await run(cleanArguments(root, filters(), false));
  output.clear();
  output.appendLine(result.stdout);
  output.show(true);
}

async function cleanAfterConfirmation(): Promise<void> {
  const root = workspaceRoot();
  if (!root) return;
  const preview = await run(cleanArguments(root, filters(), false));
  output.clear();
  output.appendLine(preview.stdout);
  output.show(true);
  const choice = await vscode.window.showWarningMessage(
    "Review the DevCleaner dry-run in the Output panel. Run the same root-scoped plan now?",
    { modal: true },
    "Clean now",
  );
  if (choice !== "Clean now") return;
  const result = await run(cleanArguments(root, filters(), true));
  output.appendLine(result.stdout);
  await scanWorkspace(false);
}

function workspaceRoot(): string | undefined {
  const folder = vscode.workspace.workspaceFolders?.find((candidate) => candidate.uri.scheme === "file");
  if (!folder) void vscode.window.showInformationMessage("Open a local folder before running DevCleaner.");
  return folder?.uri.fsPath;
}

function filters(): Filters {
  const config = vscode.workspace.getConfiguration("devclean");
  return {
    olderThan: config.get("olderThan", "7d"),
    minSize: config.get("minSize", "100MiB"),
  };
}

function run(args: string[]): Promise<{ stdout: string; stderr: string }> {
  const executable = vscode.workspace.getConfiguration("devclean").get("executable", "devclean");
  return new Promise((resolve, reject) => {
    const child = spawn(executable, args, { shell: false, windowsHide: true });
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8").on("data", (chunk: string) => { stdout += chunk; });
    child.stderr.setEncoding("utf8").on("data", (chunk: string) => { stderr += chunk; });
    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0) resolve({ stdout, stderr });
      else reject(new Error(stderr.trim() || `devclean exited with ${String(code)}`));
    });
  });
}

function formatBytes(bytes: number): string {
  return new Intl.NumberFormat(undefined, { style: "unit", unit: "megabyte", maximumFractionDigits: 1 })
    .format(bytes / 1024 / 1024);
}
