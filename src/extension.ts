import * as path from 'path';
import * as vscode from 'vscode';
import { RustFormatter, FormatterConfig, RustfmtContext } from './formatter';
import {
    applyDefaultFormatterSettings,
    DEFAULT_FORMATTER_COMMAND,
    getRustConfigurationScope,
    getRustfmtConfiguration,
    initializeDefaultFormatterPromptState,
    maybePromptDefaultFormatter
} from './defaultFormatter';
import {
    ControlCenterActionId,
    pickControlCenterAction
} from './controlCenter';
import { collectGitRustUris } from './gitSelection';

let formatter: RustFormatter;
const activeFormats = new Map<string, { tokenSource: vscode.CancellationTokenSource; promise: Promise<vscode.TextEdit[]> }>();
const MAX_FILE_SIZE_MB = 2;
let statusBarItem: vscode.StatusBarItem;
let statusBarTimer: ReturnType<typeof setTimeout> | undefined;
const BASE_STATUS_TEXT = 'rust-fmt: active';
const BASE_STATUS_TOOLTIP = 'rust-fmt is active. Click to format workspace.';
let outputChannel: vscode.OutputChannel | undefined;
const CONTROL_CENTER_COMMAND = 'rust-fmt.controlCenter';
const CONFIGURE_BEHAVIOR_COMMAND = 'rust-fmt.configureBehavior';
const OPEN_LOGS_COMMAND = 'rust-fmt.openLogs';
const MACRO_PROMPT_SUPPRESS_KEY = 'rustfmt.macroPromptSuppressed';
let extContext: vscode.ExtensionContext;
let macroPromptInProgress = false;

export function activate(context: vscode.ExtensionContext): void {
    extContext = context;
    console.log('[rust-fmt] Extension activated');
    outputChannel = vscode.window.createOutputChannel('rust-fmt');
    writeLog('Extension activated');
    initializeDefaultFormatterPromptState(context);
    const config = getFormatterConfig(vscode.window.activeTextEditor?.document.uri);
    console.log(`[rust-fmt] Config: rustfmtPath=${config.rustfmtPath}, extraArgs=${JSON.stringify(config.extraArgs)}`);
    formatter = new RustFormatter(config);
    const formattingProvider = vscode.languages.registerDocumentFormattingEditProvider('rust', {
        provideDocumentFormattingEdits(
            document: vscode.TextDocument,
            _options: vscode.FormattingOptions,
            token: vscode.CancellationToken
        ): Promise<vscode.TextEdit[]> {
            return formatDocument(document, token);
        }
    });
    const rangeFormattingProvider = vscode.languages.registerDocumentRangeFormattingEditProvider('rust', {
        async provideDocumentRangeFormattingEdits(
            document: vscode.TextDocument,
            range: vscode.Range,
            _options: vscode.FormattingOptions,
            token: vscode.CancellationToken
        ): Promise<vscode.TextEdit[]> {
            console.log(`[rust-fmt] Formatting range: lines ${range.start.line + 1}-${range.end.line + 1}`);
            if (token.isCancellationRequested) {
                return [];
            }
            const originalText = document.getText();
            const formattedText = await formatter.formatRange(document, range, token);
            if (!formattedText || formattedText === originalText) {
                return [];
            }
            const fullRange = new vscode.Range(
                document.positionAt(0),
                document.positionAt(originalText.length)
            );
            return [vscode.TextEdit.replace(fullRange, formattedText)];
        }
    });

    const formatSelectionCommand = vscode.commands.registerCommand('rust-fmt.formatSelection', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor || editor.document.languageId !== 'rust') {
            vscode.window.showWarningMessage('No active Rust file to format');
            return;
        }
        if (editor.selection.isEmpty) {
            vscode.window.showInformationMessage('Select a range of text to format, then run Format Selection.');
            return;
        }
        const edits = await vscode.commands.executeCommand<vscode.TextEdit[]>(
            'vscode.executeDocumentRangeFormatProvider',
            editor.document.uri,
            editor.selection
        );
        if (edits && edits.length > 0) {
            const edit = new vscode.WorkspaceEdit();
            edit.set(editor.document.uri, edits);
            await vscode.workspace.applyEdit(edit);
        }
    });
    const formatCommand = vscode.commands.registerCommand('rust-fmt.format', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor || editor.document.languageId !== 'rust') {
            vscode.window.showWarningMessage('No active Rust file to format');
            return;
        }
        const edits = await formatDocument(editor.document);
        if (edits.length > 0) {
            const edit = new vscode.WorkspaceEdit();
            edit.set(editor.document.uri, edits);
            await vscode.workspace.applyEdit(edit);
        }
    });
    const formatWorkspaceCommand = vscode.commands.registerCommand('rust-fmt.formatWorkspace', async () => {
        const folders = vscode.workspace.workspaceFolders;
        if (!folders || folders.length === 0) {
            vscode.window.showWarningMessage('No workspace folder open');
            return;
        }
        const include = '**/*.rs';
        const exclude = '{**/target/**,**/.git/**,**/node_modules/**,**/out/**}';
        const uris = await vscode.workspace.findFiles(include, exclude);
        if (uris.length === 0) {
            vscode.window.showInformationMessage('No Rust files found in workspace');
            return;
        }
        const dirtyDocs = new Set(
            vscode.workspace.textDocuments
                .filter((doc) => doc.languageId === 'rust' && doc.isDirty)
                .map((doc) => doc.uri.toString())
        );
        await vscode.window.withProgress(
            {
                location: vscode.ProgressLocation.Notification,
                title: 'rust-fmt: Formatting workspace',
                cancellable: true
            },
            async (progress, token) => {
                let failed = 0;
                let processed = 0;
                let dirtySkipped = 0;
                let cargoFailedCrates = 0;
                const contextCache = new Map<string, RustfmtContext>();
                const crateGroups = new Map<string, { context: RustfmtContext; uris: vscode.Uri[] }>();
                const fallbackUris: vscode.Uri[] = [];
                for (const uri of uris) {
                    if (dirtyDocs.has(uri.toString())) {
                        dirtySkipped += 1;
                        continue;
                    }
                    const dirKey = path.dirname(uri.fsPath);
                    let resolvedContext = contextCache.get(dirKey);
                    if (!resolvedContext) {
                        const workspaceFolder = vscode.workspace.getWorkspaceFolder(uri)?.uri.fsPath;
                        resolvedContext = await formatter.resolveContext(uri.fsPath, workspaceFolder);
                        contextCache.set(dirKey, resolvedContext);
                    }
                    if (resolvedContext.crateRoot) {
                        const group = crateGroups.get(resolvedContext.crateRoot);
                        if (group) {
                            group.uris.push(uri);
                        } else {
                            crateGroups.set(resolvedContext.crateRoot, { context: resolvedContext, uris: [uri] });
                        }
                    } else {
                        fallbackUris.push(uri);
                    }
                }
                const useCargoFmt = dirtyDocs.size === 0 && crateGroups.size > 0;
                if (useCargoFmt) {
                    let crateIndex = 0;
                    for (const [crateRoot, group] of crateGroups) {
                        if (token.isCancellationRequested) {
                            break;
                        }
                        crateIndex += 1;
                        progress.report({ message: `cargo fmt ${crateIndex}/${crateGroups.size}: ${path.basename(crateRoot)}` });
                        const ok = await formatter.cargoFmt(group.context, token);
                        if (!ok) {
                            cargoFailedCrates += 1;
                            fallbackUris.push(...group.uris);
                        } else {
                            for (const uri of group.uris) {
                                if (token.isCancellationRequested) { break; }
                                try {
                                    const document = await vscode.workspace.openTextDocument(uri);
                                    const text = document.getText();
                                    if (text.includes('macro_rules!')) {
                                        const edits = await formatDocument(document, token, group.context);
                                        if (edits.length > 0) {
                                            const edit = new vscode.WorkspaceEdit();
                                            edit.set(uri, edits);
                                            await vscode.workspace.applyEdit(edit);
                                        }
                                    }
                                } catch {
                                    // silent
                                }
                            }
                        }
                    }
                } else {
                    fallbackUris.push(
                        ...Array.from(crateGroups.values()).flatMap((group) => group.uris)
                    );
                }
                const totalFallback = fallbackUris.length;
                for (const uri of fallbackUris) {
                    if (token.isCancellationRequested) {
                        break;
                    }
                    const label = vscode.workspace.asRelativePath(uri);
                    progress.report({ message: `${processed + 1}/${totalFallback}: ${label}` });
                    try {
                        const document = await vscode.workspace.openTextDocument(uri);
                        const dirKey = path.dirname(document.uri.fsPath);
                        let resolvedContext = contextCache.get(dirKey);
                        if (!resolvedContext) {
                            const workspaceFolder = vscode.workspace.getWorkspaceFolder(document.uri)?.uri.fsPath;
                            resolvedContext = await formatter.resolveContext(document.uri.fsPath, workspaceFolder);
                            contextCache.set(dirKey, resolvedContext);
                        }
                        const edits = await formatDocument(document, token, resolvedContext);
                        if (edits.length > 0) {
                            const edit = new vscode.WorkspaceEdit();
                            edit.set(uri, edits);
                            const applied = await vscode.workspace.applyEdit(edit);
                            if (!applied) {
                                failed += 1;
                            }
                        }
                    } catch {
                        failed += 1;
                    }
                    processed += 1;
                }
                if (token.isCancellationRequested) {
                    vscode.window.showInformationMessage('Workspace formatting canceled.');
                    return;
                }
                if (cargoFailedCrates > 0 || failed > 0) {
                    vscode.window.showWarningMessage('Workspace formatted with errors. Check the logs for details.');
                    return;
                }
                if (dirtySkipped > 0) {
                    vscode.window.showInformationMessage('Workspace formatted. Some dirty files were skipped.');
                    return;
                }
                vscode.window.showInformationMessage('Workspace formatted.');
            }
        );
    });

    const controlCenterCommand = vscode.commands.registerCommand(CONTROL_CENTER_COMMAND, async () => {
        await openControlCenter(context);
    });

    const configureBehaviorCommand = vscode.commands.registerCommand(CONFIGURE_BEHAVIOR_COMMAND, async () => {
        await openControlCenter(context);
        updateStatusBar(vscode.window.activeTextEditor);
    });

    const openLogsCommand = vscode.commands.registerCommand(OPEN_LOGS_COMMAND, async () => {
        outputChannel?.show(true);
    });

    const formatChangedCommand = vscode.commands.registerCommand('rust-fmt.formatChanged', async () => {
        await formatGitSelection('working');
    });

    const formatStagedCommand = vscode.commands.registerCommand('rust-fmt.formatStaged', async () => {
        await formatGitSelection('staged');
    });

    const useAsDefaultFormatterCommand = vscode.commands.registerCommand(DEFAULT_FORMATTER_COMMAND, async () => {
        await applyDefaultFormatterSettings(vscode.window.activeTextEditor?.document.uri, context, { askTarget: true });
    });

    const configListener = vscode.workspace.onDidChangeConfiguration((e) => {
        if (e.affectsConfiguration('rustfmt')) {
            const newConfig = getFormatterConfig(vscode.window.activeTextEditor?.document.uri);
            formatter.updateConfig(newConfig);
        }
        const rustScope = getRustConfigurationScope(vscode.window.activeTextEditor?.document.uri);
        if (
            e.affectsConfiguration('editor.defaultFormatter', rustScope) ||
            e.affectsConfiguration('editor.formatOnSave', rustScope)
        ) {
            void maybePromptDefaultFormatter(context, vscode.window.activeTextEditor);
        }
        if (e.affectsConfiguration('rustfmt.onboarding.mode')) {
            void maybePromptDefaultFormatter(context, vscode.window.activeTextEditor);
        }
        if (
            e.affectsConfiguration('rustfmt') ||
            e.affectsConfiguration('editor.formatOnSave', rustScope)
        ) {
            updateStatusBar(vscode.window.activeTextEditor);
        }
    });
    const fileSaveListener = vscode.workspace.onDidSaveTextDocument((document) => {
        const fileName = path.basename(document.fileName);
        if (
            fileName === 'Cargo.toml' ||
            fileName === 'rustfmt.toml' ||
            fileName === '.rustfmt.toml' ||
            fileName === 'rust-toolchain' ||
            fileName === 'rust-toolchain.toml'
        ) {
            formatter.clearContextCache();
            writeLog('Context cache cleared due to config file change');
        }
    });
    statusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
    statusBarItem.tooltip = 'rust-fmt is active. Click to format workspace.';
    statusBarItem.command = 'rust-fmt.formatWorkspace';
    const editorListener = vscode.window.onDidChangeActiveTextEditor((editor) => {
        updateStatusBar(editor);
        void maybePromptDefaultFormatter(context, editor);
    });
    updateStatusBar(vscode.window.activeTextEditor);
    void maybePromptDefaultFormatter(context, vscode.window.activeTextEditor);
    context.subscriptions.push(
        formattingProvider,
        rangeFormattingProvider,
        formatSelectionCommand,
        controlCenterCommand,
        configureBehaviorCommand,
        openLogsCommand,
        formatCommand,
        formatWorkspaceCommand,
        formatChangedCommand,
        formatStagedCommand,
        useAsDefaultFormatterCommand,
        configListener,
        fileSaveListener,
        editorListener,
        statusBarItem,
        outputChannel
    );
}

async function formatDocument(
    document: vscode.TextDocument,
    token?: vscode.CancellationToken,
    resolvedContext?: RustfmtContext
): Promise<vscode.TextEdit[]> {
    const key = document.uri.toString();
    const existing = activeFormats.get(key);
    if (existing) {
        existing.tokenSource.cancel();
        try {
            await existing.promise;
        } catch {
            // Swallow cancellation error — we intentionally cancelled the previous format
        }
    }
    const tokenSource = new vscode.CancellationTokenSource();
    const externalCancellation = token?.onCancellationRequested(() => tokenSource.cancel());
    const promise = performFormat(document, tokenSource.token, resolvedContext).finally(() => {
        const current = activeFormats.get(key);
        if (current?.promise === promise) {
            activeFormats.delete(key);
        }
        externalCancellation?.dispose();
        tokenSource.dispose();
    });
    activeFormats.set(key, { tokenSource, promise });
    return promise;
}

async function performFormat(
    document: vscode.TextDocument,
    token: vscode.CancellationToken,
    resolvedContext?: RustfmtContext
): Promise<vscode.TextEdit[]> {
    console.log(`[rust-fmt] Formatting document: ${document.uri.fsPath}`);
    if (token.isCancellationRequested) {
        return [];
    }
    const originalText = document.getText();
    const sizeBytes = Buffer.byteLength(originalText, 'utf8');
    const maxBytes = MAX_FILE_SIZE_MB * 1024 * 1024;
    if (sizeBytes > maxBytes) {
        const sizeMb = (sizeBytes / (1024 * 1024)).toFixed(2);
        vscode.window.showWarningMessage(
            `[rust-fmt] File is ${sizeMb} MB, exceeds ${MAX_FILE_SIZE_MB} MB limit. Skipping format.`
        );
        return [];
    }
    if (originalText.includes('macro_rules!')) {
        void maybePromptMacroFormatting();
    }
    if (statusBarTimer) {
        clearTimeout(statusBarTimer);
        statusBarTimer = undefined;
    }
    if (statusBarItem && document.languageId === 'rust') {
        statusBarItem.text = '$(loading~spin) rust-fmt';
    }
    const startTime = Date.now();
    try {
        const formattedText = resolvedContext
            ? await formatter.formatWithContext(originalText, resolvedContext, token)
            : await formatter.format(document, token, originalText);
        if (token.isCancellationRequested) {
            resetStatusBar();
            return [];
        }
        const elapsed = Date.now() - startTime;
        if (formattedText === null || formattedText.trim() === '') {
            console.log('[rust-fmt] No formatted text returned');
            showStatusBarTime(false, elapsed);
            return [];
        }
        if (formattedText === originalText) {
            console.log('[rust-fmt] No changes needed');
            showStatusBarTime(true, elapsed);
            return [];
        }
        const fullRange = new vscode.Range(
            document.positionAt(0),
            document.positionAt(originalText.length)
        );
        console.log('[rust-fmt] Applying formatting changes');
        showStatusBarTime(true, elapsed);
        return [vscode.TextEdit.replace(fullRange, formattedText)];
    } catch {
        const elapsed = Date.now() - startTime;
        showStatusBarTime(false, elapsed);
        return [];
    }
}

function showStatusBarTime(success: boolean, elapsedMs: number): void {
    if (!statusBarItem) {
        return;
    }
    if (success) {
        statusBarItem.text = `rust-fmt: ✓ ${elapsedMs}ms`;
        statusBarItem.tooltip = `Last format: ${elapsedMs}ms. Click to format workspace.`;
    } else {
        statusBarItem.text = 'rust-fmt: ✗';
        statusBarItem.tooltip = 'Formatting failed. Click to format workspace.';
    }
    statusBarTimer = setTimeout(() => {
        resetStatusBar();
    }, 3000);
}

function resetStatusBar(): void {
    if (statusBarTimer) {
        clearTimeout(statusBarTimer);
        statusBarTimer = undefined;
    }
    if (!statusBarItem) {
        return;
    }
    statusBarItem.text = BASE_STATUS_TEXT;
    statusBarItem.tooltip = BASE_STATUS_TOOLTIP;
}

async function maybePromptMacroFormatting(): Promise<void> {
    if (macroPromptInProgress) {
        return;
    }
    if (extContext.globalState.get(MACRO_PROMPT_SUPPRESS_KEY)) {
        return;
    }
    const config = getRustfmtConfiguration();
    if (config.get<boolean>('formatMacroBodies')) {
        return;
    }
    macroPromptInProgress = true;
    try {
        const choice = await vscode.window.showInformationMessage(
            'Rust macros detected. Enable nightly macro formatting for better results?',
            'Enable',
            "Don't ask again"
        );
        if (choice === 'Enable') {
            await config.update('formatMacroBodies', true, vscode.ConfigurationTarget.Workspace);
            await config.update('formatMacroMatchers', true, vscode.ConfigurationTarget.Workspace);
            formatter.updateConfig(getFormatterConfig());
            vscode.window.showInformationMessage('Macro formatting enabled. Please save the file again to format macros.');
        } else if (choice === "Don't ask again") {
            await extContext.globalState.update(MACRO_PROMPT_SUPPRESS_KEY, true);
        }
    } finally {
        macroPromptInProgress = false;
    }
}

function getFormatterConfig(resource?: vscode.Uri): FormatterConfig {
    const config = getRustfmtConfiguration(resource);
    return {
        rustfmtPath: config.get<string>('path') || 'rustfmt',
        extraArgs: config.get<string[]>('extraArgs') || [],
        formatMacroBodies: config.get<boolean>('formatMacroBodies') || false,
        formatMacroMatchers: config.get<boolean>('formatMacroMatchers') || false,
        nativeMacroFormatter: vscode.workspace.getConfiguration('macroFormatter').get<boolean>('native') || false,
        nativeMacroFormatterPath: vscode.workspace.getConfiguration('macroFormatter').get<string>('path') || ''
    };
}

async function openControlCenter(context: vscode.ExtensionContext): Promise<void> {
    const action = await pickControlCenterAction();
    if (!action) {
        return;
    }
    await runControlCenterAction(action, context);
}

async function runControlCenterAction(action: ControlCenterActionId, context: vscode.ExtensionContext): Promise<boolean> {
    const activeResource = vscode.window.activeTextEditor?.document.uri;
    switch (action) {
        case 'runWorkspace':
            await vscode.commands.executeCommand('rust-fmt.formatWorkspace');
            return false;
        case 'runChanged':
            await vscode.commands.executeCommand('rust-fmt.formatChanged');
            return false;
        case 'runStaged':
            await vscode.commands.executeCommand('rust-fmt.formatStaged');
            return false;
        case 'setDefault':
            await applyDefaultFormatterSettings(activeResource, context, { askTarget: true });
            return false;
        case 'openLogs':
            outputChannel?.show(true);
            return false;
        case 'reloadWorkspace':
            await vscode.commands.executeCommand('workbench.action.reloadWindow');
            return false;
        default:
            return false;
    }
}

async function formatGitSelection(mode: 'working' | 'staged'): Promise<void> {
    const folders = vscode.workspace.workspaceFolders;
    if (!folders || folders.length === 0) {
        vscode.window.showWarningMessage('No workspace folder open');
        return;
    }
    const result = await collectGitRustUris(mode, folders);
    if (result.uris.length === 0) {
        const label = mode === 'staged' ? 'staged' : 'changed';
        if (result.skippedFolders.length > 0) {
            vscode.window.showWarningMessage(`No ${label} Rust files found. Some folders are not valid git worktrees.`);
            return;
        }
        vscode.window.showInformationMessage(`No ${label} Rust files found.`);
        return;
    }
    await formatSelectedUris(
        result.uris,
        mode === 'staged' ? 'rust-fmt: Formatting staged Rust files' : 'rust-fmt: Formatting changed Rust files',
        mode
    );
}

function writeLog(message: string): void {
    const line = `[${new Date().toISOString()}] ${message}`;
    outputChannel?.appendLine(line);
}

async function formatSelectedUris(
    uris: vscode.Uri[],
    title: string,
    mode: 'working' | 'staged'
): Promise<void> {
    const dirtyDocs = new Set(
        vscode.workspace.textDocuments
            .filter((doc) => doc.languageId === 'rust' && doc.isDirty)
            .map((doc) => doc.uri.toString())
    );
    await vscode.window.withProgress(
        {
            location: vscode.ProgressLocation.Notification,
            title,
            cancellable: true
        },
        async (progress, token) => {
            let failed = 0;
            let processed = 0;
            let dirtySkipped = 0;
            const contextCache = new Map<string, RustfmtContext>();
            for (const uri of uris) {
                if (token.isCancellationRequested) {
                    break;
                }
                if (dirtyDocs.has(uri.toString())) {
                    dirtySkipped += 1;
                    continue;
                }
                const label = vscode.workspace.asRelativePath(uri);
                progress.report({ message: `${processed + 1}/${uris.length}: ${label}` });
                try {
                    const document = await vscode.workspace.openTextDocument(uri);
                    const dirKey = path.dirname(document.uri.fsPath);
                    let resolvedContext = contextCache.get(dirKey);
                    if (!resolvedContext) {
                        const workspaceFolder = vscode.workspace.getWorkspaceFolder(document.uri)?.uri.fsPath;
                        resolvedContext = await formatter.resolveContext(document.uri.fsPath, workspaceFolder);
                        contextCache.set(dirKey, resolvedContext);
                    }
                    const edits = await formatDocument(document, token, resolvedContext);
                    if (edits.length > 0) {
                        const edit = new vscode.WorkspaceEdit();
                        edit.set(uri, edits);
                        const applied = await vscode.workspace.applyEdit(edit);
                        if (!applied) {
                            failed += 1;
                        }
                    }
                } catch {
                    failed += 1;
                }
                processed += 1;
            }
            if (token.isCancellationRequested) {
                vscode.window.showInformationMessage('Formatting canceled.');
                return;
            }
            if (failed > 0) {
                const label = mode === 'staged' ? 'staged' : 'changed';
                vscode.window.showWarningMessage(`Formatted ${label} Rust files with errors. Check logs for details.`);
                return;
            }
            if (dirtySkipped > 0) {
                vscode.window.showInformationMessage('Formatting complete. Some dirty files were skipped.');
                return;
            }
            const label = mode === 'staged' ? 'staged' : 'changed';
            vscode.window.showInformationMessage(`Formatted ${label} Rust files.`);
        }
    );
}

function updateStatusBar(editor?: vscode.TextEditor | null): void {
    if (!statusBarItem) {
        return;
    }
    if (editor?.document.languageId === 'rust') {
        statusBarItem.text = BASE_STATUS_TEXT;
        statusBarItem.show();
    } else {
        statusBarItem.hide();
    }
}

export function deactivate(): void { }
