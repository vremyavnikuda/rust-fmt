import * as path from 'path';
import * as vscode from 'vscode';
import { RustFormatter, RustfmtContext } from './formatter';
import { collectGitRustUris } from './gitSelection';
import { changedLineBlocks } from './lineDiff';

export type CheckScope = 'workspace' | 'changed' | 'staged';

export const RUST_FILE_INCLUDE = '**/*.rs';
export const RUST_FILE_EXCLUDE = '{**/target/**,**/.git/**,**/node_modules/**,**/out/**}';

export interface CheckSummary {
    total: number;
    unformatted: number;
    canceled: boolean;
}


async function collectUris(scope: CheckScope): Promise<vscode.Uri[]> {
    if (scope === 'workspace') {
        return vscode.workspace.findFiles(RUST_FILE_INCLUDE, RUST_FILE_EXCLUDE);
    }
    const folders = vscode.workspace.workspaceFolders;
    if (!folders || folders.length === 0) {
        return [];
    }
    const { uris } = await collectGitRustUris(scope === 'staged' ? 'staged' : 'working', folders);
    return uris;
}

/**
 * Format every file in scope in memory and mark the ones that would change.
 * Nothing is written.
 *
 * ponytail: one file at a time through the full pipeline — `cargo fmt --check`
 * would be far faster but knows nothing about macro_rules! bodies, which is the
 * whole point of this check. Batch per crate if workspace runs get too slow.
 */
export async function runCheck(
    scope: CheckScope,
    formatter: RustFormatter,
    diagnostics: vscode.DiagnosticCollection,
    progress: vscode.Progress<{ message?: string }>,
    token: vscode.CancellationToken
): Promise<CheckSummary> {
    const uris = await collectUris(scope);
    diagnostics.clear();
    const contextCache = new Map<string, RustfmtContext>();
    let unformatted = 0;
    let index = 0;
    for (const uri of uris) {
        if (token.isCancellationRequested) {
            return { total: uris.length, unformatted, canceled: true };
        }
        index += 1;
        progress.report({ message: `${index}/${uris.length}: ${vscode.workspace.asRelativePath(uri)}` });
        try {
            const document = await vscode.workspace.openTextDocument(uri);
            const dirKey = path.dirname(uri.fsPath);
            let context = contextCache.get(dirKey);
            if (!context) {
                const workspaceFolder = vscode.workspace.getWorkspaceFolder(uri)?.uri.fsPath;
                context = await formatter.resolveContext(uri.fsPath, workspaceFolder);
                contextCache.set(dirKey, context);
            }
            const original = document.getText();
            const formatted = await formatter.formatWithContext(original, context, token);
            if (formatted === null || formatted === original) {
                continue;
            }
            unformatted += 1;
            const lastLine = document.lineCount - 1;
            const fileDiagnostics = changedLineBlocks(original, formatted).map(([from, to]) => {
                const start = document.lineAt(Math.min(from, lastLine)).range.start;
                const end = document.lineAt(Math.min(to, lastLine)).range.end;
                const diagnostic = new vscode.Diagnostic(
                    new vscode.Range(start, end),
                    'Not formatted. Run rust-fmt to fix.',
                    vscode.DiagnosticSeverity.Warning
                );
                diagnostic.source = 'rust-fmt';
                return diagnostic;
            });
            diagnostics.set(uri, fileDiagnostics);
        } catch {
            // Unreadable file: nothing useful to report, leave it unmarked.
        }
    }
    return { total: uris.length, unformatted, canceled: false };
}
