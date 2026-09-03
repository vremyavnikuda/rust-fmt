import * as path from 'path';
import * as vscode from 'vscode';
import { RustFormatter, RustfmtContext } from './formatter';
import { collectGitRustUris } from './gitSelection';

export type CheckScope = 'workspace' | 'changed' | 'staged';

export const RUST_FILE_INCLUDE = '**/*.rs';
export const RUST_FILE_EXCLUDE = '{**/target/**,**/.git/**,**/node_modules/**,**/out/**}';

export interface CheckSummary {
    total: number;
    unformatted: number;
    canceled: boolean;
}

/**
 * Contiguous `[startLine, endLine]` ranges of `original` that formatting would
 * change, as 0-based inclusive line indexes.
 *
 * ponytail: common prefix/suffix trim, then an O(n*m) LCS over the differing
 * middle. Above the cap the whole middle becomes one block — swap in a Myers
 * diff if a real file ever trips it.
 */
export function changedLineBlocks(original: string, formatted: string): Array<[number, number]> {
    if (original === formatted) {
        return [];
    }
    const a = original.split('\n');
    const b = formatted.split('\n');
    let head = 0;
    while (head < a.length && head < b.length && a[head] === b[head]) {
        head += 1;
    }
    let tail = 0;
    while (
        tail < a.length - head &&
        tail < b.length - head &&
        a[a.length - 1 - tail] === b[b.length - 1 - tail]
    ) {
        tail += 1;
    }
    const left = a.slice(head, a.length - tail);
    const right = b.slice(head, b.length - tail);
    const n = left.length;
    const m = right.length;
    // Formatting only inserted lines: nothing in the original changed, so point
    // at the seam instead of reporting a clean file.
    if (n === 0) {
        return [[Math.min(head, a.length - 1), Math.min(head, a.length - 1)]];
    }
    if (n * m > 4_000_000) {
        return [[head, head + n - 1]];
    }
    const width = m + 1;
    const table = new Int32Array((n + 1) * width);
    for (let i = n - 1; i >= 0; i -= 1) {
        for (let j = m - 1; j >= 0; j -= 1) {
            table[i * width + j] = left[i] === right[j]
                ? table[(i + 1) * width + j + 1] + 1
                : Math.max(table[(i + 1) * width + j], table[i * width + j + 1]);
        }
    }
    const changed: number[] = [];
    let i = 0;
    let j = 0;
    while (i < n && j < m) {
        if (left[i] === right[j]) {
            i += 1;
            j += 1;
        } else if (table[(i + 1) * width + j] >= table[i * width + j + 1]) {
            changed.push(head + i);
            i += 1;
        } else {
            j += 1;
        }
    }
    while (i < n) {
        changed.push(head + i);
        i += 1;
    }
    if (changed.length === 0) {
        return [[Math.min(head, a.length - 1), Math.min(head, a.length - 1)]];
    }
    const blocks: Array<[number, number]> = [];
    let start = changed[0];
    let previous = changed[0];
    for (const line of changed.slice(1)) {
        if (line === previous + 1) {
            previous = line;
            continue;
        }
        blocks.push([start, previous]);
        start = line;
        previous = line;
    }
    blocks.push([start, previous]);
    return blocks;
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
