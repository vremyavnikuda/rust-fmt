import * as cp from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import * as vscode from 'vscode';

export interface FormatterConfig {
    rustfmtPath: string;
    extraArgs: string[];
    formatMacroBodies?: boolean;
    formatMacroMatchers?: boolean;
    nativeMacroFormatter?: boolean;
    nativeMacroFormatterPath?: string;
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
        return this.runCargoFmt(context.crateRoot, context.toolchain, this.config, token);
    }

    public async formatTextWithRustfmt(
        text: string,
        context: RustfmtContext,
        token?: vscode.CancellationToken,
        additionalArgs: string[] = []
    ): Promise<string | null> {
        return this.formatWithRustfmt(text, context, token, additionalArgs);
    }

    private async formatWithRustfmt(
        text: string,
        context: RustfmtContext,
        token?: vscode.CancellationToken,
        additionalArgs: string[] = []
    ): Promise<string | null> {
        if (token?.isCancellationRequested) {
            return null;
        }
        if (this.config.nativeMacroFormatter && text.includes('macro_rules!')) {
            console.log('[rust-fmt] Using native macro formatter');
            const nativeResult = await formatWithNativeMacroFormatter(text, this.config, context, token);
            if (nativeResult !== null) {
                return nativeResult;
            }
            console.log('[rust-fmt] Native macro formatter failed, falling back to TS normalize');
        }
        console.log(`[rust-fmt] Formatting with rustfmt at: ${this.config.rustfmtPath}`);
        return new Promise((resolve) => {
            const args = [...buildRustfmtArgs(this.config, context), ...additionalArgs];
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
            const normalizedText = normalizeMacroSpacing(text);
            const finalText = text.includes('macro_rules!') ? normalizeMacroBodies(normalizedText) : normalizedText;
            rustfmt.stdin.write(finalText);
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
        config?: FormatterConfig,
        token?: vscode.CancellationToken
    ): Promise<boolean> {
        console.log(`[rust-fmt] Running cargo fmt in: ${cwd}`);
        if (token?.isCancellationRequested) {
            return false;
        }
        return new Promise((resolve) => {
            const args = ['fmt', '--'];
            if (config?.formatMacroBodies) {
                args.push('--config', 'format_macro_bodies=true');
            }
            if (config?.formatMacroMatchers) {
                args.push('--config', 'format_macro_matchers=true');
            }
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

export function getNativeMacroFormatterPath(config: FormatterConfig): string | null {
    if (!config.nativeMacroFormatter) {
        return null;
    }
    if (config.nativeMacroFormatterPath) {
        return config.nativeMacroFormatterPath;
    }
    const extDir = vscode.extensions.getExtension('vremyavnikuda.rust-fmt')?.extensionPath;
    if (extDir) {
        const platform = process.platform === 'win32' ? 'win32'
            : process.platform === 'darwin' ? 'darwin'
            : 'linux';
        const arch = process.arch;
        const binaryName = process.platform === 'win32' ? 'rust-fmt-mf.exe' : 'rust-fmt-mf';
        const bundled = path.join(extDir, 'bin', `${platform}-${arch}`, binaryName);
        try {
            if (fs.existsSync(bundled)) {
                return bundled;
            }
        } catch {
            // ignore
        }
    }
    return null;
}

export async function formatWithNativeMacroFormatter(
    text: string,
    config: FormatterConfig,
    context: RustfmtContext,
    token?: vscode.CancellationToken
): Promise<string | null> {
    const binaryPath = getNativeMacroFormatterPath(config);
    if (!binaryPath) {
        return null;
    }
    if (token?.isCancellationRequested) {
        return null;
    }
    return new Promise((resolve) => {
        const args: string[] = [];
        args.push('--edition', context.edition || '2021');
        args.push('--rustfmt-path', config.rustfmtPath);
        if (context.configPath) {
            args.push('--config-path', context.configPath);
        }
        const env = { ...process.env };
        if (context.toolchain && !env.RUSTUP_TOOLCHAIN) {
            env.RUSTUP_TOOLCHAIN = context.toolchain;
        }
        const proc = cp.spawn(binaryPath, args, {
            cwd: context.cwd,
            shell: false,
            env,
        });
        let stdout = '';
        let stderr = '';
        let settled = false;
        const timeoutMs = 30000;
        const cancelSubscription = token?.onCancellationRequested(() => {
            proc.kill();
            finish(null);
        });
        const finish = (result: string | null) => {
            if (settled) return;
            settled = true;
            clearTimeout(timeout);
            cancelSubscription?.dispose();
            resolve(result);
        };
        const timeout = setTimeout(() => {
            proc.kill();
            finish(null);
        }, timeoutMs);
        proc.stdout.on('data', (data) => { stdout += data.toString(); });
        proc.stderr.on('data', (data) => { stderr += data.toString(); });
        proc.on('error', (err) => {
            if (settled) return;
            console.error(`[rust-fmt] Native macro formatter error: ${err.message}`);
            finish(null);
        });
        proc.on('close', (code) => {
            if (settled) return;
            if (code === 0) {
                finish(stdout || null);
            } else {
                console.error(`[rust-fmt] Native macro formatter exited with code ${code}: ${stderr}`);
                finish(null);
            }
        });
        if (token?.isCancellationRequested) {
            finish(null);
            return;
        }
        proc.stdin.write(text);
        proc.stdin.end();
    });
}

function buildRustfmtArgs(config: FormatterConfig, context: RustfmtContext): string[] {
    const args: string[] = ['--emit', 'stdout'];
    const normalizedExtraArgs = config.extraArgs ?? [];
    const hasArg = (name: string): boolean =>
        normalizedExtraArgs.some((arg) => arg === name || arg.startsWith(`${name}=`));
    if (context.configPath && !hasArg('--config-path')) {
        args.push('--config-path', context.configPath);
    }
    if (context.edition && !hasArg('--edition')) {
        args.push('--edition', context.edition);
    }
    if (config.formatMacroBodies) {
        args.push('--config', 'format_macro_bodies=true');
    }
    if (config.formatMacroMatchers) {
        args.push('--config', 'format_macro_matchers=true');
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

export function normalizeMacroSpacing(text: string): string {
    return text.split('\n').map(line => {
        const indent = line.match(/^( +|\t*)/)?.[0] || '';
        const rest = line.slice(indent.length);
        const processed = rest
            .replace(/([[({]) {2,}/g, '$1')
            .replace(/ {2,}/g, ' ');
        return indent + processed;
    }).join('\n');
}

export function normalizeMacroBodies(text: string): string {
    const lines = text.split('\n');
    const result = [...lines];
    let i = 0;
    while (i < lines.length) {
        const line = lines[i];
        if (!/^\s*macro_rules!\s/.test(line)) { i++; continue; }
        let depth = 0;
        let end = i;
        for (let j = i; j < lines.length; j++) {
            depth += countChar(lines[j], '{') - countChar(lines[j], '}');
            if (depth === 0 && j > i) { end = j; break; }
        }
        const macroText = lines.slice(i, end + 1).join('\n');
        let armNestDepth = 0;
        let armLineStart = -1;
        let armLines: string[] = [];
        let armBodyLineStart = -1;
        let armLineIndent = 0;
        let currentLineIdx = 0;
        for (let pos = 0; pos < macroText.length; pos++) {
            const ch = macroText[pos];
            if (ch === '\n') {
                currentLineIdx++;
                if (armNestDepth > 0 && currentLineIdx !== armLineStart) {
                    armLines.push(lines[i + currentLineIdx]);
                }
                continue;
            }
            if (armNestDepth > 0) {
                if (ch === '{') {
                    armNestDepth++;
                } else if (ch === '}') {
                    armNestDepth--;
                    if (armNestDepth === 0) {
                        if (armLines.length === 0 || armLines[armLines.length - 1] !== lines[i + currentLineIdx]) {
                            armLines.push(lines[i + currentLineIdx]);
                        }
                        const bodyLines = armLines.slice(1);
                        const innerCount = bodyLines.length - 1;
                        const expectedIndent = armLineIndent + 4;
                        if (innerCount >= 1) {
                            let nestLevel = 0;
                            for (let bi = 0; bi < innerCount; bi++) {
                                const bl = bodyLines[bi];
                                const bt = bl.trimStart();
                                if (bt.length === 0) { continue; }
                                // Pre-indent: close repetition and braces first
                                if (/\)[+*]/.test(bt)) {
                                    nestLevel = Math.max(0, nestLevel - 1);
                                }
                                nestLevel = Math.max(0, nestLevel - countChar(bt, '}'));
                                // Compute indent
                                const newIndent = expectedIndent + nestLevel * 4;
                                // Post-indent: open braces and repetition
                                nestLevel += countChar(bt, '{');
                                if (/^\$\(/.test(bt)) {
                                    nestLevel++;
                                }
                                const resultIdx = i + armBodyLineStart + bi;
                                if (resultIdx < result.length) {
                                    const oldIndent = bl.length - bt.length;
                                    if (newIndent !== oldIndent) {
                                        result[resultIdx] = ' '.repeat(newIndent) + bt;
                                    }
                                }
                            }
                        }
                        armNestDepth = 0;
                        armLines = [];
                        armBodyLineStart = -1;
                    }
                }
                continue;
            }
            if (ch === '=' && pos + 1 < macroText.length && macroText[pos + 1] === '>') {
                let scanPos = pos + 2;
                while (scanPos < macroText.length && (macroText[scanPos] === ' ' || macroText[scanPos] === '\t' || macroText[scanPos] === '\n')) {
                    if (macroText[scanPos] === '\n') { currentLineIdx++; }
                    scanPos++;
                }
                if (scanPos < macroText.length && macroText[scanPos] === '{') {
                    armNestDepth = 1;
                    armLineStart = currentLineIdx;
                    armLines = [lines[i + currentLineIdx]];
                    armBodyLineStart = currentLineIdx + 1;
                    const armLine = lines[i + currentLineIdx];
                    armLineIndent = armLine.length - armLine.trimStart().length;
                    pos = scanPos;
                    continue;
                }
            }
        }
        i = end + 1;
    }
    return result.join('\n');
}

function countChar(s: string, ch: string): number {
    let c = 0;
    for (let idx = 0; idx < s.length; idx++) {
        if (s[idx] === ch) { c++; }
    }
    return c;
}
