import * as path from 'path';
import * as vscode from 'vscode';
import { RustFormatter, FormatterConfig, RustfmtContext } from './formatter';

let formatter: RustFormatter;
const activeFormats = new Map<string, { tokenSource: vscode.CancellationTokenSource; promise: Promise<vscode.TextEdit[]> }>();
const MAX_FILE_SIZE_MB = 2;
let statusBarItem: vscode.StatusBarItem;
const DEFAULT_FORMATTER_ID = 'vremyavnikuda.rust-fmt';
const DEFAULT_FORMATTER_COMMAND = 'rust-fmt.useAsDefaultFormatter';
const DEFAULT_FORMATTER_LAST_OBSERVED_KEY = 'rustfmt.defaultFormatterLastObserved';
const DEFAULT_FORMATTER_PROMPT_SUPPRESS_KEY = 'rustfmt.defaultFormatterPromptSuppress';
const DEFAULT_FORMATTER_NONE_SENTINEL = '__none__';
const DEFAULT_FORMATTER_PROMPT_STATE_VERSION_KEY = 'rustfmt.defaultFormatterPromptStateVersion';
const DEFAULT_FORMATTER_PROMPT_STATE_VERSION = 1;
let promptInProgress = false;
let ignoreLastObservedOnce = false;

export function activate(context: vscode.ExtensionContext) {
    console.log('[rust-fmt] Extension activated');

    const promptStateVersion = context.workspaceState.get<number>(DEFAULT_FORMATTER_PROMPT_STATE_VERSION_KEY);
    if (promptStateVersion !== DEFAULT_FORMATTER_PROMPT_STATE_VERSION) {
        ignoreLastObservedOnce = true;
        void context.workspaceState.update(DEFAULT_FORMATTER_PROMPT_STATE_VERSION_KEY, DEFAULT_FORMATTER_PROMPT_STATE_VERSION);
        void context.workspaceState.update(DEFAULT_FORMATTER_LAST_OBSERVED_KEY, undefined);
    }

    const config = getFormatterConfig();
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

    const useAsDefaultFormatterCommand = vscode.commands.registerCommand(DEFAULT_FORMATTER_COMMAND, async () => {
        await applyDefaultFormatterSettings(vscode.window.activeTextEditor?.document.uri, context, { askTarget: true });
    });

    const configListener = vscode.workspace.onDidChangeConfiguration((e) => {
        if (e.affectsConfiguration('rustfmt')) {
            const newConfig = getFormatterConfig();
            formatter.updateConfig(newConfig);
        }

        const rustScope = getRustConfigurationScope(vscode.window.activeTextEditor?.document.uri);
        if (
            e.affectsConfiguration('editor.defaultFormatter', rustScope) ||
            e.affectsConfiguration('editor.formatOnSave', rustScope)
        ) {
            void maybePromptDefaultFormatter(context, vscode.window.activeTextEditor);
        }
    });

    statusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
    statusBarItem.text = 'rust-fmt: active';
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
        formatCommand,
        formatWorkspaceCommand,
        useAsDefaultFormatterCommand,
        configListener,
        editorListener,
        statusBarItem
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
            // REVIEW:Ignore errors from canceled/failed format runs.
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

    const formattedText = resolvedContext
        ? await formatter.formatWithContext(originalText, resolvedContext, token)
        : await formatter.format(document, token, originalText);

    if (token.isCancellationRequested) {
        return [];
    }

    if (formattedText === null || formattedText.trim() === '') {
        console.log('[rust-fmt] No formatted text returned');
        return [];
    }

    if (formattedText === originalText) {
        console.log('[rust-fmt] No changes needed');
        return [];
    }

    const fullRange = new vscode.Range(
        document.positionAt(0),
        document.positionAt(originalText.length)
    );

    console.log('[rust-fmt] Applying formatting changes');
    return [vscode.TextEdit.replace(fullRange, formattedText)];
}

function getFormatterConfig(): FormatterConfig {
    const config = vscode.workspace.getConfiguration('rustfmt');

    return {
        rustfmtPath: config.get<string>('path') || 'rustfmt',
        extraArgs: config.get<string[]>('extraArgs') || []
    };
}

function updateStatusBar(editor?: vscode.TextEditor | null): void {
    if (!statusBarItem) {
        return;
    }

    if (editor?.document.languageId === 'rust') {
        statusBarItem.show();
    } else {
        statusBarItem.hide();
    }
}

async function maybePromptDefaultFormatter(
    context: vscode.ExtensionContext,
    editor?: vscode.TextEditor | null
): Promise<void> {
    if (!editor || editor.document.languageId !== 'rust') {
        return;
    }

    if (context.globalState.get(DEFAULT_FORMATTER_PROMPT_SUPPRESS_KEY)) {
        return;
    }

    const currentDefaultFormatter = getCurrentDefaultFormatter(editor.document);
    const currentKey = currentDefaultFormatter ?? DEFAULT_FORMATTER_NONE_SENTINEL;
    let lastObserved = context.workspaceState.get<string>(DEFAULT_FORMATTER_LAST_OBSERVED_KEY);
    if (ignoreLastObservedOnce) {
        lastObserved = undefined;
        ignoreLastObservedOnce = false;
    }

    if (currentDefaultFormatter === DEFAULT_FORMATTER_ID) {
        await context.workspaceState.update(DEFAULT_FORMATTER_LAST_OBSERVED_KEY, currentKey);
        return;
    }

    if (promptInProgress) {
        return;
    }

    if (lastObserved === currentKey) {
        return;
    }

    await context.workspaceState.update(DEFAULT_FORMATTER_LAST_OBSERVED_KEY, currentKey);
    promptInProgress = true;

    try {
        let message = 'No default formatter is configured for Rust. Switch to rust-fmt?';
        if (currentDefaultFormatter) {
            const otherFormatterLabel = resolveFormatterLabel(currentDefaultFormatter);
            message = otherFormatterLabel
                ? `Rust is currently formatted by "${otherFormatterLabel}". Switch to rust-fmt?`
                : 'A different default formatter is set for Rust. Switch to rust-fmt?';
        }

        const choice = await vscode.window.showInformationMessage(
            message,
            'Switch',
            "Don't ask again"
        );

        if (choice === 'Switch') {
            await applyDefaultFormatterSettings(editor.document.uri, context, { askTarget: true });
        } else if (choice === "Don't ask again") {
            await context.globalState.update(DEFAULT_FORMATTER_PROMPT_SUPPRESS_KEY, true);
        }
    } finally {
        promptInProgress = false;
    }
}

function getCurrentDefaultFormatter(document: vscode.TextDocument): string | undefined {
    const config = vscode.workspace.getConfiguration('editor', getRustConfigurationScope(document.uri));
    return config.get<string>('defaultFormatter') || undefined;
}

function resolveFormatterLabel(extensionId: string): string | undefined {
    const extension = vscode.extensions.getExtension(extensionId);
    if (!extension) {
        return extensionId;
    }

    const packageJson = extension.packageJSON ?? {};
    return packageJson.displayName || packageJson.name || extensionId;
}

async function applyDefaultFormatterSettings(
    resource?: vscode.Uri,
    context?: vscode.ExtensionContext,
    options?: { askTarget?: boolean; target?: vscode.ConfigurationTarget }
): Promise<void> {
    const scope = getRustConfigurationScope(resource);
    const config = vscode.workspace.getConfiguration('editor', scope);
    let target = options?.target;
    if (options?.askTarget) {
        target = await pickConfigurationTarget();
        if (target === undefined) {
            return;
        }
    }
    if (target === undefined) {
        const fallbackTarget = getConfigurationTarget(resource);
        target = resolveConfigurationTarget(config, fallbackTarget);
    }
    try {
        await config.update('defaultFormatter', DEFAULT_FORMATTER_ID, target, true);
        await config.update('formatOnSave', true, target, true);
        if (context) {
            await context.workspaceState.update(DEFAULT_FORMATTER_LAST_OBSERVED_KEY, DEFAULT_FORMATTER_ID);
        }
    } catch (err) {
        console.error('[rust-fmt] Failed to update formatter settings', err);
    }
}

function getConfigurationTarget(resource?: vscode.Uri): vscode.ConfigurationTarget {
    const folders = vscode.workspace.workspaceFolders;
    if (!folders || folders.length === 0) {
        return vscode.ConfigurationTarget.Global;
    }

    if (resource) {
        const folder = vscode.workspace.getWorkspaceFolder(resource);
        if (folder) {
            return vscode.ConfigurationTarget.WorkspaceFolder;
        }
    }

    return vscode.ConfigurationTarget.Workspace;
}

function resolveConfigurationTarget(
    config: vscode.WorkspaceConfiguration,
    fallback: vscode.ConfigurationTarget
): vscode.ConfigurationTarget {
    const inspected = config.inspect<string>('defaultFormatter');
    if (!inspected) {
        return fallback;
    }

    if (
        inspected.workspaceFolderLanguageValue !== undefined ||
        inspected.workspaceFolderValue !== undefined
    ) {
        return vscode.ConfigurationTarget.WorkspaceFolder;
    }

    if (
        inspected.workspaceLanguageValue !== undefined ||
        inspected.workspaceValue !== undefined
    ) {
        return vscode.ConfigurationTarget.Workspace;
    }

    if (
        inspected.globalLanguageValue !== undefined ||
        inspected.globalValue !== undefined
    ) {
        return vscode.ConfigurationTarget.Global;
    }

    return fallback;
}

async function pickConfigurationTarget(): Promise<vscode.ConfigurationTarget | undefined> {
    const folders = vscode.workspace.workspaceFolders;
    if (!folders || folders.length === 0) {
        return vscode.ConfigurationTarget.Global;
    }

    const selection = await vscode.window.showQuickPick(
        [
            { label: 'Global', description: 'Apply to all workspaces' },
            { label: 'Workspace', description: 'Apply to this workspace only' }
        ],
        {
            placeHolder: 'Set rust-fmt as default formatter for Rust',
            canPickMany: false
        }
    );

    if (!selection) {
        return undefined;
    }

    return selection.label === 'Workspace'
        ? vscode.ConfigurationTarget.Workspace
        : vscode.ConfigurationTarget.Global;
}

function getRustConfigurationScope(resource?: vscode.Uri): vscode.ConfigurationScope {
    if (resource) {
        return { uri: resource, languageId: 'rust' };
    }

    return { languageId: 'rust' };
}

export function deactivate() { }
