import * as cp from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';

const GIT_DIFF_FILTER = 'ACMRT';

export async function collectGitRustUris(
    mode: 'working' | 'staged',
    folders: readonly vscode.WorkspaceFolder[]
): Promise<{ uris: vscode.Uri[]; skippedFolders: string[] }> {
    const deduped = new Map<string, vscode.Uri>();
    const skippedFolders: string[] = [];

    for (const folder of folders) {
        let output = '';
        try {
            output = await runGitDiff(folder.uri.fsPath, mode);
        } catch {
            skippedFolders.push(folder.name);
            continue;
        }

        const changedPaths = output
            .split(/\r?\n/)
            .map((value) => value.trim())
            .filter((value) => value.length > 0);

        for (const relativePath of changedPaths) {
            if (!relativePath.endsWith('.rs')) {
                continue;
            }

            const fullPath = path.resolve(folder.uri.fsPath, relativePath);
            if (!await fileExists(fullPath)) {
                continue;
            }

            const uri = vscode.Uri.file(fullPath);
            deduped.set(uri.toString(), uri);
        }
    }

    return {
        uris: Array.from(deduped.values()),
        skippedFolders
    };
}

async function runGitDiff(rootPath: string, mode: 'working' | 'staged'): Promise<string> {
    return new Promise((resolve, reject) => {
        const args = [
            '-C',
            rootPath,
            'diff',
            '--name-only',
            `--diff-filter=${GIT_DIFF_FILTER}`
        ];
        if (mode === 'staged') {
            args.push('--cached');
        }

        cp.execFile('git', args, { maxBuffer: 1024 * 1024 }, (error, stdout, stderr) => {
            if (error) {
                reject(new Error(stderr || error.message));
                return;
            }
            resolve(stdout);
        });
    });
}

async function fileExists(filePath: string): Promise<boolean> {
    try {
        await fs.promises.access(filePath, fs.constants.F_OK);
        return true;
    } catch {
        return false;
    }
}
