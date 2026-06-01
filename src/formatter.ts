import * as cp from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';

export interface FormatterConfig {
    rustfmtPath: string;
    extraArgs: string[];
}

export interface RustfmtContext {
    cwd: string | undefined;
    crateRoot?: string;
    configPath?: string;
    edition?: string;
    toolchain?: string;
}

export class RustFormatter {
    private config: FormatterConfig;
    private contextCache = new Map<string, { ctx: RustfmtContext; mtime: number }>();

    constructor(config: FormatterConfig) {
        this.config = config;
    }

    public async format(
        document: vscode.TextDocument,
        token?: vscode.CancellationToken,
        textOverride?: string
    ): Promise<string | null> {
        const text = textOverride ?? document.getText();
        const filePath = document.uri.fsPath;
        const fileDir = path.dirname(filePath);
        const workspaceFolder = vscode.workspace.getWorkspaceFolder(document.uri)?.uri.fsPath;

        let context: RustfmtContext;
        try {
            const stat = await fs.promises.stat(fileDir);
            const cached = this.contextCache.get(filePath);
            if (cached && cached.mtime === stat.mtimeMs) {
                context = cached.ctx;
            } else {
                context = await this.resolveContext(filePath, workspaceFolder);
                this.contextCache.set(filePath, { ctx: context, mtime: stat.mtimeMs });
            }
        } catch {
            context = await this.resolveContext(filePath, workspaceFolder);
        }

        return this.formatWithRustfmt(text, context, token);
    }

    public async formatWithContext(
        text: string,
        context: RustfmtContext,
        token?: vscode.CancellationToken
    ): Promise<string | null> {
        return this.formatWithRustfmt(text, context, token);
    }

    public async formatRange(
        document: vscode.TextDocument,
        range: vscode.Range,
        token?: vscode.CancellationToken
    ): Promise<string | null> {
        const text = document.getText();
        const filePath = document.uri.fsPath;
        const workspaceFolder = vscode.workspace.getWorkspaceFolder(document.uri)?.uri.fsPath;
        const context = await this.resolveContext(filePath, workspaceFolder);

        // rustfmt uses 1-based line numbers
        const startLine = range.start.line + 1;
        const endLine = range.end.line + 1;

        const fileLines = JSON.stringify([{ file: 'stdin', range: [startLine, endLine] }]);
        const additionalArgs = ['--file-lines', fileLines];

        return this.formatWithRustfmt(text, context, token, additionalArgs);
    }

    public async resolveContext(filePath: string, workspaceFolder?: string): Promise<RustfmtContext> {
        return resolveRustfmtContext(filePath, workspaceFolder);
    }

    public async cargoFmt(context: RustfmtContext, token?: vscode.CancellationToken): Promise<boolean> {
        if (!context.crateRoot) {
            return false;
        }

        return this.runCargoFmt(context.crateRoot, context.toolchain, token);
    }

    private async formatWithRustfmt(
        text: string,
        context: RustfmtContext,
        token?: vscode.CancellationToken,
        additionalArgs: string[] = []
    ): Promise<string | null> {
        console.log(`[rust-fmt] Formatting with rustfmt at: ${this.config.rustfmtPath}`);

        if (token?.isCancellationRequested) {
            return null;
        }

        return new Promise((resolve) => {
            const args = [...buildRustfmtArgs(this.config.extraArgs, context), ...additionalArgs];
            console.log(`[rust-fmt] Running: ${this.config.rustfmtPath} ${args.join(' ')}`);

            const env = { ...process.env };
            if (context.toolchain && !env.RUSTUP_TOOLCHAIN) {
                env.RUSTUP_TOOLCHAIN = context.toolchain;
                console.log(`[rust-fmt] Using toolchain override: ${context.toolchain}`);
            }

            const rustfmt = cp.spawn(this.config.rustfmtPath, args, {
                cwd: context.cwd,
                shell: false,
                env
            });

            let stdout = '';
            let stderr = '';
            let settled = false;
            const timeoutMs = 10000;
            const cancelSubscription = token?.onCancellationRequested(() => {
                console.warn('[rust-fmt] Formatting canceled, killing rustfmt process');
                rustfmt.kill();
                finish(null);
            });

            const finish = (result: string | null) => {
                if (settled) {
                    return;
                }
                settled = true;
                clearTimeout(timeout);
                cancelSubscription?.dispose();
                resolve(result);
            };

            const timeout = setTimeout(() => {
                console.error('[rust-fmt] Timeout: rustfmt took too long, killing process');
                rustfmt.kill();
                finish(null);
            }, timeoutMs);

            rustfmt.stdout.on('data', (data) => {
                stdout += data.toString();
            });

            rustfmt.stderr.on('data', (data) => {
                stderr += data.toString();
            });

            rustfmt.on('error', (err) => {
                if (settled) {
                    return;
                }
                console.error('[rust-fmt] Error:', err);
                vscode.window.showErrorMessage(`Failed to run rustfmt: ${err.message}`);
                finish(null);
            });

            rustfmt.on('close', (code) => {
                if (settled) {
                    return;
                }
                console.log(`[rust-fmt] Process exited with code: ${code}`);
                if (stderr) {
                    console.log(`[rust-fmt] stderr: ${stderr}`);
                }
                
                if (code === 0) {
                    if (!stdout || stdout.trim() === '') {
                        console.log('[rust-fmt] Warning: empty output from rustfmt');
                        finish(null);
                    } else {
                        console.log(`[rust-fmt] Successfully formatted, output length: ${stdout.length}`);
                        finish(stdout);
                    }
                } else {
                    vscode.window.showErrorMessage(`rustfmt exited with code ${code}: ${stderr}`);
                    finish(null);
                }
            });

            if (token?.isCancellationRequested) {
                finish(null);
                return;
            }

            rustfmt.stdin.write(text);
            rustfmt.stdin.end();
        });
    }

    public updateConfig(config: FormatterConfig): void {
        this.config = config;
        this.contextCache.clear();
    }

    public clearContextCache(): void {
        this.contextCache.clear();
    }

    private async runCargoFmt(
        cwd: string,
        toolchain?: string,
        token?: vscode.CancellationToken
    ): Promise<boolean> {
        console.log(`[rust-fmt] Running cargo fmt in: ${cwd}`);

        if (token?.isCancellationRequested) {
            return false;
        }

        return new Promise((resolve) => {
            const args = ['fmt'];
            const env = { ...process.env };
            if (toolchain && !env.RUSTUP_TOOLCHAIN) {
                env.RUSTUP_TOOLCHAIN = toolchain;
                console.log(`[rust-fmt] Using toolchain override for cargo fmt: ${toolchain}`);
            }

            const cargo = cp.spawn('cargo', args, {
                cwd,
                shell: false,
                env
            });

            let stderr = '';
            let settled = false;
            const timeoutMs = 60000;
            const cancelSubscription = token?.onCancellationRequested(() => {
                console.warn('[rust-fmt] cargo fmt canceled, killing process');
                cargo.kill();
                finish(false);
            });

            const finish = (result: boolean) => {
                if (settled) {
                    return;
                }
                settled = true;
                clearTimeout(timeout);
                cancelSubscription?.dispose();
                resolve(result);
            };

            const timeout = setTimeout(() => {
                console.error('[rust-fmt] Timeout: cargo fmt took too long, killing process');
                cargo.kill();
                finish(false);
            }, timeoutMs);

            cargo.stderr.on('data', (data) => {
                stderr += data.toString();
            });

            cargo.on('error', (err) => {
                if (settled) {
                    return;
                }
                console.error('[rust-fmt] cargo fmt error:', err);
                vscode.window.showErrorMessage(`Failed to run cargo fmt: ${err.message}`);
                finish(false);
            });

            cargo.on('close', (code) => {
                if (settled) {
                    return;
                }
                if (code === 0) {
                    console.log('[rust-fmt] cargo fmt completed successfully');
                    finish(true);
                } else {
                    console.error(`[rust-fmt] cargo fmt exited with code ${code}`);
                    if (stderr) {
                        console.log(`[rust-fmt] cargo fmt stderr: ${stderr}`);
                    }
                    vscode.window.showErrorMessage(`cargo fmt exited with code ${code}: ${stderr}`);
                    finish(false);
                }
            });
        });
    }
}

function buildRustfmtArgs(extraArgs: string[], context: RustfmtContext): string[] {
    const args: string[] = ['--emit', 'stdout'];
    const normalizedExtraArgs = extraArgs ?? [];
    const hasArg = (name: string): boolean =>
        normalizedExtraArgs.some((arg) => arg === name || arg.startsWith(`${name}=`));

    if (context.configPath && !hasArg('--config-path')) {
        args.push('--config-path', context.configPath);
    }

    if (context.edition && !hasArg('--edition')) {
        args.push('--edition', context.edition);
    }

    args.push(...normalizedExtraArgs);
    return args;
}

async function resolveRustfmtContext(filePath: string, workspaceFolder?: string): Promise<RustfmtContext> {
    const fileDir = path.dirname(filePath);

    const [cargoTomlPath, configPath, toolchainPath] = await Promise.all([
        findNearestFile(fileDir, ['Cargo.toml'], workspaceFolder),
        findNearestFile(fileDir, ['rustfmt.toml', '.rustfmt.toml'], workspaceFolder),
        findNearestFile(fileDir, ['rust-toolchain.toml', 'rust-toolchain'], workspaceFolder)
    ]);

    const [edition, toolchain] = await Promise.all([
        cargoTomlPath ? readEditionFromCargoToml(cargoTomlPath) : Promise.resolve(undefined),
        toolchainPath ? readToolchainFromFile(toolchainPath) : Promise.resolve(undefined)
    ]);

    const crateRoot = cargoTomlPath ? path.dirname(cargoTomlPath) : workspaceFolder;

    return {
        cwd: crateRoot ?? workspaceFolder ?? fileDir,
        crateRoot: cargoTomlPath ? path.dirname(cargoTomlPath) : undefined,
        configPath: configPath ?? undefined,
        edition,
        toolchain
    };
}

async function findNearestFile(
    startDir: string,
    candidateNames: string[],
    stopDir?: string
): Promise<string | null> {
    let current = path.resolve(startDir);
    const stop = stopDir ? path.resolve(stopDir) : undefined;
    const stopNormalized = stop
        ? (process.platform === 'win32' ? stop.toLowerCase() : stop)
        : undefined;

    let done = false;
    while (!done) {
        for (const name of candidateNames) {
            const candidate = path.join(current, name);
            try {
                await fs.promises.access(candidate, fs.constants.F_OK);
                return candidate;
            } catch {
                // Not found in this directory.
            }
        }

        const currentKey = process.platform === 'win32' ? current.toLowerCase() : current;
        if (stopNormalized && currentKey === stopNormalized) {
            done = true;
            continue;
        }

        const parent = path.dirname(current);
        if (parent === current) {
            done = true;
            continue;
        }
        current = parent;
    }

    return null;
}

async function readEditionFromCargoToml(cargoTomlPath: string): Promise<string | undefined> {
    try {
        const contents = await fs.promises.readFile(cargoTomlPath, 'utf8');
        const match = contents.match(/^\s*edition\s*=\s*["'](\d{4})["']\s*(#.*)?$/m);
        return match?.[1];
    } catch {
        return undefined;
    }
}

async function readToolchainFromFile(toolchainPath: string): Promise<string | undefined> {
    try {
        const contents = await fs.promises.readFile(toolchainPath, 'utf8');
        const channelMatch = contents.match(/^\s*channel\s*=\s*["']([^"']+)["']\s*(#.*)?$/m);
        if (channelMatch?.[1]) {
            return channelMatch[1];
        }

        const lines = contents.split(/\r?\n/);
        for (const line of lines) {
            const trimmed = line.trim();
            if (!trimmed || trimmed.startsWith('#')) {
                continue;
            }
            return trimmed.replace(/^["']|["']$/g, '');
        }

        return undefined;
    } catch {
        return undefined;
    }
}


